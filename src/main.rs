use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::fs;
use std::path::Path;
use log::{info, warn};
use tokio::{signal, sync::Mutex, sync::MutexGuard};
use chrono::{DateTime, Utc};
use teloxide::{
    dispatching::{dialogue, dialogue::InMemStorage, UpdateHandler},
    prelude::*,
    types::{KeyboardButton, KeyboardMarkup},
    utils::command::BotCommands,
};
use serde::{Serialize, Deserialize};
use env_logger;

use bot_structure::*;
use add_expenses::*;
use add_category::*;
use delete_category::*;
use cleanup_expenses::*;
use show_expenses::*;

pub mod bot_structure;
pub mod add_expenses;
pub mod add_category;
pub mod delete_category;
pub mod cleanup_expenses;
pub mod show_expenses;

const DATA_FILE_PATH: &str = "users_data.json";
const DEFAULT_OTHER_CATEGORY: &str = "Другое";
const MAX_ITEMS_IN_MESSAGE: usize = 100;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    info!("");
    info!("---------------------------");

    let token = String::from("8148320925:AAEh0-L5Wb29tPUAYcaNsZWQ5_MN5CxsF18");

    let bot = Bot::new(token);
    let user_data = Arc::new(Mutex::new(load_user_data().unwrap_or_default()));

    let _dispatch_task = tokio::spawn(async move {
        Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![InMemStorage::<State>::new(), user_data])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    });

    signal::ctrl_c().await?;

    Ok(())
}

fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(dptree::case![Command::Start].endpoint(handle_start_command))
        .branch(dptree::case![Command::Help].endpoint(handle_help_command))
        .branch(dptree::case![Command::AddExpense].endpoint(start_add_expense))
        .branch(dptree::case![Command::AddNewCategory].endpoint(start_add_category))
        .branch(dptree::case![Command::DeleteCategory].endpoint(start_delete_category))
        .branch(dptree::case![Command::ClearAllExpenses].endpoint(start_cleanup_expenses))
        .branch(dptree::case![Command::AllExpenses].endpoint(show_all_expenses))
        .branch(dptree::case![Command::TotalExpenses].endpoint(show_total_expenses))
        .branch(dptree::case![Command::ExpensesByCategory].endpoint(show_expenses_by_category))
        .endpoint(handle_command);

    let message_handler = Update::filter_message()
        .branch(command_handler)
        .branch(dptree::case![State::Default].endpoint(handle_message_expense))
        .branch(dptree::case![State::AddExpense].endpoint(handle_message_expense))
        .branch(dptree::case![State::SelectCategory { pending_expense }].endpoint(handle_message_on_select_category))
        .branch(dptree::case![State::ConfirmAddExpense { pending_expense, category }].endpoint(handle_message_on_confirm_expense))
        .branch(dptree::case![State::AddCategory].endpoint(handle_message_on_add_category))
        .branch(dptree::case![State::ConfirmAddCategory { category }].endpoint(handle_message_on_confirm_category))
        .branch(dptree::case![State::DeleteCategory].endpoint(handle_message_on_delete_category))
        .branch(dptree::case![State::ConfirmDeleteCategory { category }].endpoint(handle_message_on_confirm_delete_category))
        .branch(dptree::case![State::ConfirmCleanupExpenses].endpoint(handle_message_on_confirm_cleanup_expenses));

    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(message_handler)
}

fn load_user_data() -> Result<HashMap<UserId, UserData>, Box<dyn Error>> {
    info!("Loading data...");
    if !Path::new(DATA_FILE_PATH).exists() {
        info!("No data file - creating new");
        return Ok(HashMap::new());
    }

    info!("Found data file, reading...");
    let file_content = fs::read_to_string(DATA_FILE_PATH)?;
    let user_data: HashMap<UserId, UserData> = serde_json::from_str(&file_content)?;
    Ok(user_data)
}

pub async fn save_user_data(user_data: &HashMap<UserId, UserData>) -> Result<(), Box<dyn Error>> {
    info!("Saving data...");
    let json = serde_json::to_string_pretty(&user_data)?;
    fs::write(DATA_FILE_PATH, json)?;
    Ok(())
}

pub fn get_user_entry<'a>(user_data: &'a mut MutexGuard<'_, HashMap<UserId, UserData>>, user_id: UserId) -> &'a mut UserData {
    let user_entry = user_data.entry(user_id).or_default();
    if user_entry.categories.is_empty() {
        user_entry.categories.push(DEFAULT_OTHER_CATEGORY.to_string());
    }
    return user_entry;
}

pub async fn enter_default_state(bot: Bot, chat_id: ChatId, dialogue: MyDialogue) -> HandlerResult {
    bot.send_message(chat_id,
        "Привет! Я бот для учёта расходов. Начните с команды /addexpense, или напишите трату в формате: продукт цена (например, молоко 100)")
        .await?;
    dialogue.update(State::Default).await?;
    Ok(())
}

async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command
) -> HandlerResult {
    if let Some(text) = msg.text() {
        info!("Received unparsed command: {}", text);
    }
    else {
        info!("Received unknown unparsed command");
    }

    bot.send_message(msg.chat.id,
        "Не поддерживаем пока такую команду")
        .await?;
    Ok(())
}

async fn handle_start_command(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    enter_default_state(bot, msg.chat.id, dialogue).await?;
    Ok(())
}

async fn handle_help_command(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
    Ok(())
}
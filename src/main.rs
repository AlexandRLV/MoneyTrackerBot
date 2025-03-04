use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::fs;
use std::path::Path;
use log::{info, warn};
use tokio::{signal, sync::Mutex};
use chrono::{DateTime, Utc};
use teloxide::{prelude::*,
    types::{CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, MessageKind, UpdateKind, User},
    utils::command::BotCommands};
use serde::{Serialize, Deserialize};
use env_logger;

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Expense {
    description: String,
    amount: f64,
    category: String,
    #[serde_as(as = "serde_with::TimestampSecondsWithFrac<String>")]
    date: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct UserData {
    expenses: Vec<Expense>,
    requested_clear: bool,
    pending_expense: Option<(String, f64)>,
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Доступные команды")]
enum Command {
    #[command(description = "Показать это сообщение")]
    Help,
    #[command(description = "Показать приветственное сообщение")]
    Start,
    #[command(description = "Очистить список трат")]
    ClearExpenses,
    #[command(description = "Вывести список всех трат")]
    AllExpenses,
    #[command(description = "Вывести сумму трат")]
    TotalExpenses,
    #[command(description = "Вывести сумму трат по категориям")]
    ExpensesByCategory,
}

const CATEGORIES: [&str; 6] = [
    "Продукты",
    "Транспорт",
    "Рестораны",
    "Квартира",
    "Одежда",
    // "В рубли",
    // "Уроки",
    // "Ништяки",
    "Другое",
];

const DATA_FILE_PATH: &str = "users_data.json";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    info!("");
    info!("---------------------------");

    let bot = Bot::from_env();
    let user_data = Arc::new(Mutex::new(load_user_data().unwrap_or_default()));

    let handler = dptree::entry()
    .branch(
        Update::filter_message()
                .filter_command::<Command>()
                .endpoint(handle_command),
    )
    .branch(
        Update::filter_message()
            .inspect(|| info!("Received message"))
            .endpoint(handle_message),
    )
    .branch(
        Update::filter_callback_query()
            .inspect(|| info!("Received callback"))
            .endpoint(handle_callback),
    );

    let _dispatch_task = tokio::spawn(async move {
        Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![user_data])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    });

    signal::ctrl_c().await?;

    Ok(())
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

async fn save_user_data(user_data: &HashMap<UserId, UserData>) -> Result<(), Box<dyn Error>> {
    info!("Saving data...");
    let json = serde_json::to_string_pretty(&user_data)?;
    fs::write(DATA_FILE_PATH, json)?;
    Ok(())
} 

async fn handle_command(bot: Bot, msg: Message, cmd: Command, user_data: Arc<Mutex<HashMap<UserId, UserData>>>) -> ResponseResult<()> {
    let user_id = msg.from().unwrap().id;

    if let Some(text) = msg.text() {
        info!("Received command: {}", text);
    }
    else {
        info!("Received unknown command");
    }

    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
        },
        Command::Start => {
            bot.send_message(msg.chat.id,
                "Привет! Я бот для учёта расходов. Пишите свои траты в формате: описание цена, например: продукты 15.5")
                .await?;
        },
        Command::ClearExpenses => {
            clear_user_expenses(&bot, &msg, &user_data, user_id).await?;
        }
        Command::AllExpenses => {
            show_all_expenses(&bot, &msg, &user_data, user_id).await?;
        },
        Command::TotalExpenses => {
            show_total_expenses(&bot, &msg, &user_data, user_id).await?;
        },
        Command::ExpensesByCategory => {
            show_expenses_by_category(&bot, &msg, &user_data, user_id).await?;
        }
    }

    Ok(())
}

async fn handle_message(bot: Bot, msg: Message, user_data: Arc<Mutex<HashMap<UserId, UserData>>>) -> ResponseResult<()> {
    if let Some(text) = msg.text() {
        info!("Received message: {}", text);
        let user_id = msg.from().unwrap().id;

        if let Some((description, amount)) = parse_expense(text) {
            info!("Parsed expense: {}, {}", description, amount);
            let mut data = user_data.lock().await;
            let user_entry = data.entry(user_id).or_default();
            user_entry.pending_expense = Some((description.clone(), amount));

            if let Err(e) = save_user_data(&data).await {
                warn!("Save data error: {}", e);
            }

            let markup = create_category_keyboard();

            bot.send_message(
                msg.chat.id,
                format!("Трата '{}' на сумму {:.2} добавлена. Выберите категорию:", description, amount)
            )
            .reply_markup(markup)
            .await?;
        } else {
            bot.send_message(msg.chat.id, "Пожалуйста, укажите трату в формате 'описание сумма', например: 'продукты 15.5'").await?;
        }
    }

    Ok(())
}

async fn handle_callback(bot: Bot, query: CallbackQuery, user_data: Arc<Mutex<HashMap<UserId, UserData>>>) -> ResponseResult<()> {
    info!("Handling callback");

    let user_id = query.from.id;
    let chat_id = if let Some(message) = &query.message {
        message.chat().id
    } else {
        return Ok(());
    };

    info!("Got user and chat id");
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();

    if let Some(category) = query.data {
        if user_entry.requested_clear {
            info!("Requested clearing data");
            if  category == "ConfirmClear" {
                info!("Callback is for clearing");

                data.remove_entry(&user_id);
                info!("Removed user entry from data");
                
                if let Err(e) = save_user_data(&data).await {
                    warn!("Save data error: {}", e);
                }

                bot.send_message(chat_id, "Ваши траты были удалены").await?;
            }
            else {
                bot.send_message(chat_id, "Отменяем удаление трат").await?;
            }
        } else if let Some((description, amount)) = user_entry.pending_expense.take() {
            info!("Got pending expense {}, {}", description, amount);

            let expense = Expense {
                description,
                amount,
                category: category.clone(),
                date: Utc::now()
            };

            info!("Created expense");

            user_entry.expenses.push(expense);
            
            if let Err(e) = save_user_data(&data).await {
                warn!("Save data error: {}", e);
            }

            info!("Answering callback");
            bot.answer_callback_query(query.id).await?;
            bot.send_message(
                chat_id,
                format!("Трата добавлена в категорию '{}'", category)
            ).await?;
        } else {
            info!("No pending expense");
            bot.answer_callback_query(query.id).await?;
            bot.send_message(chat_id,
                "Ошибка: информация о трате не найдена. Попробуйте записать её ещё раз"
            ).await?;
        }
    } else if user_entry.requested_clear {
        user_entry.requested_clear = false;
    }

    Ok(())
}

async fn clear_user_expenses(bot: &Bot, msg: &Message, user_data: &Arc<Mutex<HashMap<UserId, UserData>>>, user_id: UserId) -> ResponseResult<()> {
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();

    user_entry.requested_clear = true;

    bot.send_message(msg.chat.id, "Вы действительно хотите удалить свои траты?").await?;

    Ok(())
}

async fn show_all_expenses(bot: &Bot, msg: &Message, user_data: &Arc<Mutex<HashMap<UserId, UserData>>>, user_id: UserId) -> ResponseResult<()> {
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();

    if user_entry.expenses.is_empty() {
        bot.send_message(msg.chat.id, "Вы пока не записали ни одну трату").await?;
        return Ok(());
    }

    let mut message = String::from("Ваши траты:\n\n");

    for expense in &user_entry.expenses {
        message.push_str(&format!(
            "Трата: {}, сумма: {:.2}, категория: {}, дата: {}\n",
            expense.description,
            expense.amount,
            expense.category,
            expense.date.format("%d.%m.%y %H:%M")
        ));
    }
    
    bot.send_message(msg.chat.id, message).await?;
    Ok(())
}

async fn show_total_expenses(bot: &Bot, msg: &Message, user_data: &Arc<Mutex<HashMap<UserId, UserData>>>, user_id: UserId) -> ResponseResult<()> {
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();

    if user_entry.expenses.is_empty() {
        bot.send_message(msg.chat.id, "Вы пока не записали ни одну трату").await?;
        return Ok(());
    }

    let total: f64 = user_entry.expenses.iter().map(|e| e.amount).sum();

    bot.send_message(msg.chat.id, format!("Общая сумма трат: {:.2}", total)).await?;
    Ok(())
}

async fn show_expenses_by_category(bot: &Bot, msg: &Message, user_data: &Arc<Mutex<HashMap<UserId, UserData>>>, user_id: UserId) -> ResponseResult<()> {
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();

    if user_entry.expenses.is_empty() {
        bot.send_message(msg.chat.id, "Вы пока не записали ни одну трату").await?;
        return Ok(());
    }

    let mut category_totals: HashMap<String, f64> = HashMap::new();

    for expense in &user_entry.expenses {
        *category_totals.entry(expense.category.clone()).or_default() += expense.amount;
    }

    let mut message = String::from("Траты по категориям: \n\n");

    for (category, total) in &category_totals {
        message.push_str(&format!("{}: {:.2}\n", category, total));
    }

    bot.send_message(msg.chat.id, message).await?;
    Ok(())
}

fn create_category_keyboard() -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    let mut row: Vec<InlineKeyboardButton> = Vec::new();

    for (i, category) in CATEGORIES.iter().enumerate() {
        let button = InlineKeyboardButton::callback(category.to_string(), category.to_string());

        row.push(button);

        if (i + 1) % 2 == 0 || i == CATEGORIES.len() - 1 {
            keyboard.push(row);
            row = Vec::new();
        }
    }

    InlineKeyboardMarkup::new(keyboard)
}

fn parse_expense(text: &str) -> Option<(String, f64)> {
    let words: Vec<&str> = text.split_whitespace().collect();

    if words.len() < 2 {
        return None;
    }

    if let Ok(amount) = words.last().unwrap().parse::<f64>() {
        let description = words[..words.len() - 1].join(" ");
        return Some((description, amount));
    }

    None
}
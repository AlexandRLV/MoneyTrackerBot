use crate::{add_category::category_is_valid, *};

pub async fn start_add_expense(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    bot.send_message(msg.chat.id,
        "Введите трату в формате: описание цена, например: продукты 15.5")
        .await?;
    dialogue.update(State::AddExpense).await?;
    Ok(())
}

pub async fn handle_message_expense(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    if let Some(text) = msg.text() {
        info!("Received message: {}", text);
        let user_id = msg.from().unwrap().id;

        if let Some((description, amount)) = parse_expense(text) {
            info!("Parsed expense: {}, {}", description, amount);

            let mut data = user_data.lock().await;
            let user_entry = data.entry(user_id).or_default();
            
            send_select_category(bot, msg.chat.id, user_entry, dialogue, description, amount).await?;
        } else {
            bot.send_message(msg.chat.id, "Пожалуйста, укажите трату в формате 'описание сумма', например: 'продукты 15.5'").await?;
        }
    }

    Ok(())
}

pub async fn handle_message_on_select_category(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    pending_expense: (String, f64),
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    let user_id = msg.from().unwrap().id;
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();
    let (description, amount) = pending_expense;

    if let Some(category) = msg.text() {
        let category = category.to_owned();
        if category_is_valid(&category, user_entry) {
            send_confirm_category(bot, msg.chat.id, description, amount, category, dialogue).await?;
            return Ok(());
        }
    }

    send_select_category(bot, msg.chat.id, user_entry, dialogue, description, amount).await?;
    Ok(())
}

pub async fn handle_callback_on_select_category(
    bot: Bot,
    query: CallbackQuery,
    dialogue: MyDialogue,
    pending_expense: (String, f64),
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    let user_id = query.from.id;
    let chat_id = if let Some(message) = &query.message {
        message.chat().id
    } else {
        dialogue.update(State::Default).await?;
        return Ok(());
    };

    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();
    let (description, amount) = pending_expense;

    bot.answer_callback_query(query.id).await?;
    if let Some(answer) = query.data {
        if category_is_valid(&answer, user_entry) {
            send_confirm_category(bot, chat_id, description, amount, answer, dialogue).await?;
            return Ok(());
        } else if answer == "Back" {
            send_select_category(bot, chat_id, user_entry, dialogue, description, amount).await?;
            return Ok(());
        } else if answer == "Cancel" {
            send_back_to_default(bot, chat_id, dialogue).await?;
            return Ok(());
        }
    } else {
        send_select_category(bot, chat_id, user_entry, dialogue, description, amount).await?;
    }

    Ok(())
}

pub async fn handle_message_on_confirm_expense(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    pending_expense: (String, f64),
    category: String
) -> HandlerResult {
    let (description, amount) = pending_expense;
    bot.send_message(msg.chat.id, "Пожалуйста, подтвердите или отмените добавление траты").await?;
    send_confirm_category(bot, msg.chat.id, description, amount, category, dialogue).await?;
    Ok(())
}

pub async fn handle_callback_on_confirm_expense(
    bot: Bot,
    query: CallbackQuery,
    dialogue: MyDialogue,
    pending_expense: (String, f64),
    category: String,
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    let user_id = query.from.id;
    let chat_id = if let Some(message) = &query.message {
        message.chat().id
    } else {
        dialogue.update(State::Default).await?;
        return Ok(());
    };

    let (description, amount) = pending_expense;
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();

    bot.answer_callback_query(query.id).await?;
    if let Some(answer) = query.data {
        if answer == "Confirm" {
            let expense = Expense {
                description,
                amount,
                category: category.clone(),
                date: Utc::now()
            };

            user_entry.expenses.push(expense);
            
            if let Err(e) = save_user_data(&data).await {
                warn!("Save data error: {}", e);
            }

            bot.send_message(
                chat_id,
                format!("Трата добавлена в категорию '{}'", category)
            ).await?;
            dialogue.update(State::Default).await?;
            return Ok(());
        } else if answer == "Back" {
            send_select_category(bot, chat_id, user_entry, dialogue, description, amount).await?;
            return Ok(());
        } else if answer == "Cancel" {
            send_back_to_default(bot, chat_id, dialogue).await?;
            return Ok(());
        }
    }

    send_select_category(bot, chat_id, user_entry, dialogue, description, amount).await?;
    Ok(())
}

async fn send_select_category(
    bot: Bot,
    chat_id: ChatId,
    user_entry: &mut UserData,
    dialogue: MyDialogue,
    description: String,
    amount: f64
) -> HandlerResult {
    if user_entry.categories.len() > 0 {
        let markup = create_category_keyboard(&user_entry.categories);

        bot.send_message(
            chat_id,
            format!("Вы ввели трату '{}' на сумму {:.2}. Выберите категорию из списка или введите новую:", description, amount)
        )
        .reply_markup(markup)
        .await?;
    } else {
        bot.send_message(
            chat_id,
            format!("Вы ввели трату '{}' на сумму {:.2}. Вы ещё не добавили ни одной категории, введите новую:", description, amount)
        )
        .await?;
    }

    dialogue.update(State::SelectCategory { pending_expense: (description, amount) }).await?;
    Ok(())
}

async fn send_confirm_category(
    bot: Bot,
    chat_id: ChatId,
    description: String,
    amount: f64,
    category: String,
    dialogue: MyDialogue
) -> HandlerResult {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    let mut row: Vec<InlineKeyboardButton> = Vec::new();
    row.push(InlineKeyboardButton::callback("Назад", "NO"));
    row.push(InlineKeyboardButton::callback("Подтвердить", "YES"));
    keyboard.push(row);

    let markup = InlineKeyboardMarkup::new(keyboard);

    bot.send_message(
        chat_id,
        format!("Подтвердите добавление траты '{}' на сумму {:.2} в категорию {}", description, amount, category)
    )
    .reply_markup(markup)
    .await?;
    dialogue.update(State::ConfirmAddExpense{ pending_expense: (description, amount), category: category.to_owned() }).await?;
    Ok(())
}

async fn send_back_to_default(bot: Bot, chat_id: ChatId, dialogue: MyDialogue) -> HandlerResult {
    bot.send_message(chat_id, "Добавление траты отменено").await?;
    dialogue.update(State::Default).await?;
    Ok(())
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

fn create_category_keyboard(categories: &Vec<String>) -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    let mut row: Vec<InlineKeyboardButton> = Vec::new();

    for (i, category) in categories.iter().enumerate() {
        let button = InlineKeyboardButton::callback(category.to_string(), category.to_string());

        row.push(button);

        if (i + 1) % 2 == 0 || i == categories.len() - 1 {
            keyboard.push(row);
            row = Vec::new();
        }
    }

    InlineKeyboardMarkup::new(keyboard)
}
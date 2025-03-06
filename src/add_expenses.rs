use crate::*;

pub async fn start_add_expense(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    info!("Got command /addexpense");
    bot.send_message(msg.chat.id,
        "Введите трату в формате: описание цена, например: продукты 15.5")
        .await?;

    info!("Changing state to AddExpense");
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

        if let Some((description, amount)) = parse_expense(text) {
            info!("Parsed expense: {}, {}", description, amount);

            let user_id = msg.from.unwrap().id;
            let mut data = user_data.lock().await;
            let user_entry = data.entry(user_id).or_default();
            
            send_select_category(bot, msg.chat.id, user_entry, dialogue, description, amount).await?;
        } else {
            info!("Expense didn't parsed");
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
    info!("Got message with category");
    let user_id = msg.from.as_ref().unwrap().id;
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();
    let (description, amount) = pending_expense;

    if let Some(category) = msg.text() {
        let category = category.to_owned();
        info!("Got category: {}", category);
        send_confirm_expense(bot, msg.chat.id, description, amount, category, dialogue).await?;
        return Ok(());
    }

    info!("Go back to select category");
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
    info!("Got callback when selecting category");
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
        info!("Got query answer: {}", answer);
        if answer == "Back" {
            info!("Got Back request");
            send_select_category(bot, chat_id, user_entry, dialogue, description, amount).await?;
            return Ok(());
        } else if answer == "Cancel" {
            info!("Got Cancel request");
            send_back_to_default(bot, chat_id, dialogue).await?;
            return Ok(());
        } else {
            info!("Category is valid");
            send_confirm_expense(bot, chat_id, description, amount, answer, dialogue).await?;
            return Ok(());
        }
    }

    info!("Go back to select category");
    send_select_category(bot, chat_id, user_entry, dialogue, description, amount).await?;
    Ok(())
}

pub async fn handle_message_on_confirm_expense(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    pending_expense: (String, f64),
    category: String
) -> HandlerResult {
    info!("Got message on confirm expense, send confirm again");
    let (description, amount) = pending_expense;
    bot.send_message(msg.chat.id, "Пожалуйста, подтвердите или отмените добавление траты").await?;
    send_confirm_expense(bot, msg.chat.id, description, amount, category, dialogue).await?;
    Ok(())
}

pub async fn handle_callback_on_confirm_expense(
    bot: Bot,
    query: CallbackQuery,
    dialogue: MyDialogue,
    (pending_expense, category): ((String, f64), String),
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    info!("Got callback on confirm expense");
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
        info!("Got query data");
        if answer == "Confirm" {
            info!("Got Confirm request");
            let expense = Expense {
                description,
                amount,
                category: category.clone(),
                date: Utc::now()
            };

            user_entry.expenses.push(expense);
            
            if !user_entry.categories.contains(&category) {
                user_entry.categories.push(category.clone());
            }
            
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
            info!("Got Back request");
            send_select_category(bot, chat_id, user_entry, dialogue, description, amount).await?;
            return Ok(());
        } else if answer == "Cancel" {
            info!("Got Cancel request");
            send_back_to_default(bot, chat_id, dialogue).await?;
            return Ok(());
        }

        info!("Unknown callback");
    }

    info!("Go back to select category");
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
    info!("Sending select category");
    let markup = create_category_keyboard(&user_entry.categories);
    if user_entry.categories.len() > 0 {
        info!("User has categories");
        bot.send_message(
            chat_id,
            format!("Вы ввели трату '{}' на сумму {:.2}. Выберите категорию из списка или введите новую:", description, amount)
        )
        .reply_markup(markup)
        .await?;
    } else {
        info!("User doesn't have any category");
        bot.send_message(
            chat_id,
            format!("Вы ввели трату '{}' на сумму {:.2}. Вы ещё не добавили ни одной категории, введите новую:", description, amount)
        )
        .await?;
    }

    info!("Changing state to SelectCategory");
    dialogue.update(State::SelectCategory { pending_expense: (description, amount) }).await?;
    Ok(())
}

async fn send_confirm_expense(
    bot: Bot,
    chat_id: ChatId,
    description: String,
    amount: f64,
    category: String,
    dialogue: MyDialogue
) -> HandlerResult {
    info!("Sending confirm expense");
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    let mut row: Vec<InlineKeyboardButton> = Vec::new();
    row.push(InlineKeyboardButton::callback("Отменить", "Cancel"));
    row.push(InlineKeyboardButton::callback("Назад", "Back"));
    row.push(InlineKeyboardButton::callback("Подтвердить", "Confirm"));
    keyboard.push(row);

    let markup = InlineKeyboardMarkup::new(keyboard);

    bot.send_message(
        chat_id,
        format!("Подтвердите добавление траты '{}' на сумму {:.2} в категорию {}", description, amount, category)
    )
    .reply_markup(markup)
    .await?;

    info!("Changing state to ConfirmAddExpense");
    dialogue.update(State::ConfirmAddExpense{ pending_expense: (description, amount), category: category.to_owned() }).await?;
    Ok(())
}

async fn send_back_to_default(bot: Bot, chat_id: ChatId, dialogue: MyDialogue) -> HandlerResult {
    bot.send_message(chat_id, "Добавление траты отменено").await?;
    info!("Changing state to Default");
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
    info!("Creating category keyboard");
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

    row.push(InlineKeyboardButton::callback("Отменить", "Cancel"));
    row.push(InlineKeyboardButton::callback("Назад", "Back"));
    keyboard.push(row);

    InlineKeyboardMarkup::new(keyboard)
}
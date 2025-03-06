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

            let user_id = msg.from.as_ref().unwrap().id;
            let mut data = user_data.lock().await;
            let user_entry = data.entry(user_id).or_default();
            
            send_select_category(bot, msg.chat.id, user_entry, dialogue, description, amount).await?;
            return Ok(());
        }
    }

    info!("Expense didn't parsed");
    bot.send_message(msg.chat.id, "Пожалуйста, укажите трату в формате 'описание сумма', например: 'продукты 15.5'").await?;
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
    let text = if let Some(text) = msg.text() {
        text.to_owned()
    } else {
        info!("Message text not parsed");
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    };

    if text == "Назад" {
        info!("Go back to default");
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    let user_id = msg.from.as_ref().unwrap().id;
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();
    let (description, amount) = pending_expense;

    if let Ok(id) = text.parse::<usize>() {
        info!("Parsed id: {}", id);
        if id >= user_entry.categories.len() {
            info!("No such id");
            bot.send_message(msg.chat.id, "Нет категории с таким id").await?;
            send_select_category(bot, msg.chat.id, user_entry, dialogue, description, amount).await?;
            return Ok(());
        }

        let category = &user_entry.categories[id];
        info!("Got category by id: {}", category);
        send_confirm_expense(bot, msg.chat.id, description, amount, category.to_string(), dialogue).await?;
        return Ok(());
    }
    
    info!("Got category: {}", text);
    send_confirm_expense(bot, msg.chat.id, description, amount, text, dialogue).await?;
    return Ok(());

    // info!("Go back to select category");
    // send_select_category(bot, msg.chat.id, user_entry, dialogue, description, amount).await?;
    // Ok(())
}

pub async fn handle_message_on_confirm_expense(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    (pending_expense, category): ((String, f64), String),
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    info!("Got message on confirm expense");
    let text = if let Some(text) = msg.text() {
        text.to_owned()
    } else {
        info!("Message text not parsed");
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    };

    if text == "Отменить" {
        info!("Cancel add expense");
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    let user_id = msg.from.as_ref().unwrap().id;
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();
    let (description, amount) = pending_expense;

    if text == "Назад" {
        info!("Go back to select category");
        send_select_category(bot, msg.chat.id, user_entry, dialogue, description, amount).await?;
        return Ok(());
    }

    if text == "Да" {
        info!("Adding expense");
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
            msg.chat.id,
            format!("Трата добавлена в категорию '{}'", category)
        ).await?;
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }
    
    info!("Not parsed text");
    bot.send_message(msg.chat.id, "Пожалуйста, подтвердите или отмените добавление траты, используя предложенные варианты").await?;
    send_confirm_expense(bot, msg.chat.id, description, amount, category, dialogue).await?;
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
    
    if user_entry.categories.len() == 0 {
        info!("User doesn't have any category");
        let keyboard = KeyboardMarkup::new(
            vec![vec![KeyboardButton::new("Назад")]])
            .resize_keyboard()
            .one_time_keyboard();
        
        bot.send_message(
            chat_id,
            format!("Вы ввели трату '{}' на сумму {:.2}. Вы ещё не добавили ни одной категории, введите новую:", description, amount))
            .reply_markup(keyboard)
            .await?;
        dialogue.update(State::SelectCategory { pending_expense: (description, amount) }).await?;
        return Ok(());
    }

    info!("User has categories");
    let keyboard = KeyboardMarkup::new(
        vec![vec![KeyboardButton::new("Отменить"), KeyboardButton::new("Назад"), KeyboardButton::new("Да")]])
        .resize_keyboard()
        .one_time_keyboard();

    let mut message = String::from("Ваши категории:\n\n");

    for (i, category) in user_entry.categories.iter().enumerate() {
        message.push_str(&format!(
            "Id: {}, название: {}",
            i,
            category));
    }

    bot.send_message(chat_id, message).await?;

    bot.send_message(
        chat_id,
        format!("Вы ввели трату '{}' на сумму {:.2}. Введите Id или название категории из списка, или введите название новой категории", description, amount)
    )
    .reply_markup(keyboard)
    .await?;

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
use crate::*;

pub async fn start_add_category(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    info!("Got command /addcategory");
    send_add_category(bot, msg.chat.id, dialogue).await?;
    Ok(())
}

pub async fn handle_message_on_add_category(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    let text = if let Some(text) = msg.text() {
        text.to_owned()
    } else {
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    };

    if text == "Назад" {
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    let user_id = msg.from.as_ref().unwrap().id;
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();
    if user_entry.categories.contains(&text) {
        bot.send_message(msg.chat.id,
            "Такая категория уже добавлена")
            .await?;
        send_add_category(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    send_confirm_category(bot, msg.chat.id, dialogue, text).await?;
    Ok(())
}

pub async fn handle_message_on_confirm_category(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    category: String,
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    let text = if let Some(text) = msg.text() {
        text.to_owned()
    } else {
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    };

    if text == "Изменить" {
        send_add_category(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    if text == "Нет" {
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    if text == "Да" {
        let user_id = msg.from.as_ref().unwrap().id;
        let mut data = user_data.lock().await;
        let user_entry = data.entry(user_id).or_default();
        if user_entry.categories.contains(&category) {
            bot.send_message(msg.chat.id,
                "Такая категория уже добавлена")
                .await?;
            send_add_category(bot, msg.chat.id, dialogue).await?;
            return Ok(());
        }

        user_entry.categories.push(category);
        if let Err(e) = save_user_data(&data).await {
            warn!("Save data error: {}", e);
        }

        bot.send_message(msg.chat.id,
            "Категория успешно добавлена")
            .await?;

        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    bot.send_message(msg.chat.id,
        "Не понимаю вас")
        .await?;
    
    send_confirm_category(bot, msg.chat.id, dialogue, category).await?;
    Ok(())
}

async fn send_add_category(bot: Bot, chat_id: ChatId, dialogue: MyDialogue) -> HandlerResult {
    let keyboard = KeyboardMarkup::new(
        vec![vec![KeyboardButton::new("Назад")]])
        .resize_keyboard()
        .one_time_keyboard();

    bot.send_message(chat_id,
        "Введите название для новой категории трат:")
        .reply_markup(keyboard)
        .await?;

    info!("Changing state to AddCategory");
    dialogue.update(State::AddCategory).await?;
    Ok(())
}

async fn send_confirm_category(bot: Bot, chat_id: ChatId, dialogue: MyDialogue, category: String) -> HandlerResult {
    let keyboard = KeyboardMarkup::new(
        vec![vec![KeyboardButton::new("Изменить"), KeyboardButton::new("Нет"), KeyboardButton::new("Да")]])
        .resize_keyboard()
        .one_time_keyboard();

    bot.send_message(chat_id,
        format!("Подтвердите добавление новой категории: {}", category))
        .reply_markup(keyboard)
        .await?;

    info!("Changing state to ConfirmAddCategory");
    dialogue.update(State::ConfirmAddCategory { category }).await?;
    Ok(())
}
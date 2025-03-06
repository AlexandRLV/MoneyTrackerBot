use crate::*;

pub async fn start_delete_category(bot: Bot, msg: Message, dialogue: MyDialogue, user_data: Arc<Mutex<HashMap<UserId, UserData>>>) -> HandlerResult {
    info!("Got command /deletecategory");
    let user_id = msg.from.as_ref().unwrap().id;
    let mut data = user_data.lock().await;
    let user_entry = data.entry(user_id).or_default();
    send_delete_category(bot, msg.chat.id, dialogue, user_entry).await?;
    Ok(())
}

pub async fn handle_message_on_delete_category(
    bot: Bot, msg: Message, dialogue: MyDialogue, user_data: Arc<Mutex<HashMap<UserId, UserData>>>
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

    if let Ok(id) = text.parse::<usize>() {
        if id >= user_entry.categories.len() {
            bot.send_message(msg.chat.id, "Нет категории с таким id").await?;
            send_delete_category(bot, msg.chat.id, dialogue, user_entry).await?;
            return Ok(());
        }

        let category = &user_entry.categories[id];
        send_confirm_delete_category(bot, msg.chat.id, dialogue, category.to_string()).await?;
        return Ok(());
    }
    
    if !user_entry.categories.contains(&text) {
        bot.send_message(msg.chat.id, "Такой категории не существует").await?;
        send_delete_category(bot, msg.chat.id, dialogue, user_entry).await?;
        return Ok(());
    }
    
    send_confirm_delete_category(bot, msg.chat.id, dialogue, text).await?;
    Ok(())
}

pub async fn handle_message_on_confirm_delete_category(
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

    if text == "Нет" {
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    if text == "Да" {
        let user_id = msg.from.as_ref().unwrap().id;
        let mut data = user_data.lock().await;
        let user_entry = data.entry(user_id).or_default();
        
        if let Some(pos) = user_entry.categories.iter().position(|c| c == &category) {
            user_entry.categories.remove(pos);
        }

        let mut was_expenses = false;
        for expense in &mut user_entry.expenses {
            if expense.category == category {
                was_expenses = true;
                expense.category = DEFAULT_OTHER_CATEGORY.to_string();
            }
        }

        if let Err(e) = save_user_data(&data).await {
            warn!("Save data error: {}", e);
        }

        let message = if was_expenses {
            "Категория успешно удалена, все траты перемещены в категорию 'Другое'"
        } else {
            "Категория успешно удалена, трат в этой категории не было"
        };
        bot.send_message(msg.chat.id, message).await?;
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    bot.send_message(msg.chat.id, "Пожалуйста, подтвердите удаление категории, выбрав одну из предоставленных опций").await?;
    send_confirm_delete_category(bot, msg.chat.id, dialogue, category).await?;
    Ok(())
}

async fn send_delete_category(
    bot: Bot,
    chat_id: ChatId,
    dialogue: MyDialogue, 
    user_entry: &mut UserData,
) -> HandlerResult {    
    if user_entry.categories.is_empty() {
        bot.send_message(chat_id, "У вас нет категорий для удаления").await?;
        enter_default_state(bot, chat_id, dialogue).await?;
        return Ok(());
    }

    let keyboard = KeyboardMarkup::new(
        vec![vec![KeyboardButton::new("Назад")]])
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
    bot.send_message(chat_id, "Введите Id или название категории, которую хотите удалить:").reply_markup(keyboard).await?;

    dialogue.update(State::DeleteCategory).await?;
    Ok(())
}

async fn send_confirm_delete_category(bot: Bot, chat_id: ChatId, dialogue: MyDialogue, category: String) -> HandlerResult {
    let keyboard = KeyboardMarkup::new(
        vec![vec![KeyboardButton::new("Нет"), KeyboardButton::new("Да")]])
        .resize_keyboard()
        .one_time_keyboard();

    bot.send_message(chat_id, format!("Вы уверены, что хотите удалить категорию '{}'? Все траты из этой категории перейдут в категорию 'Другое'", category))
        .reply_markup(keyboard)
        .await?;

    dialogue.update(State::ConfirmDeleteCategory { category }).await?;
    Ok(())
}
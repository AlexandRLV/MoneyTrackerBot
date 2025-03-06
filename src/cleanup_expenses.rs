use crate::*;

pub async fn start_cleanup_expenses(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    info!("Got command /clearallexpenses");
    send_confirm_cleanup_expenses(bot, msg.chat.id, dialogue).await?;
    Ok(())
}

pub async fn handle_message_on_confirm_cleanup_expenses(
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

    if text == "Нет" {
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    if text == "Да" {
        let user_id = msg.from.as_ref().unwrap().id;
        let mut data = user_data.lock().await;
        let user_entry = get_user_entry(&mut data, user_id);
        
        user_entry.expenses.clear();

        if let Err(e) = save_user_data(&data).await {
            warn!("Save data error: {}", e);
        }

        bot.send_message(msg.chat.id, "Все траты успешно удалены").await?;
        enter_default_state(bot, msg.chat.id, dialogue).await?;
        return Ok(());
    }

    bot.send_message(msg.chat.id, "Не понимаю вас").await?;
    send_confirm_cleanup_expenses(bot, msg.chat.id, dialogue).await?;
    Ok(())
}

async fn send_confirm_cleanup_expenses(bot: Bot, chat_id: ChatId, dialogue: MyDialogue) -> HandlerResult {
    let keyboard = KeyboardMarkup::new(
        vec![vec![KeyboardButton::new("Нет"), KeyboardButton::new("Да")]])
        .resize_keyboard()
        .one_time_keyboard();

    bot.send_message(chat_id, "Вы уверены, что хотите удалить ВСЕ траты? Это действие нельзя отменить.")
        .reply_markup(keyboard)
        .await?;

    dialogue.update(State::ConfirmCleanupExpenses).await?;
    Ok(())
}
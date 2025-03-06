use crate::*;

pub async fn show_all_expenses(
    bot: Bot,
    msg: Message,
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>,
) -> HandlerResult {
    let user_id = msg.from.as_ref().unwrap().id;
    let mut data = user_data.lock().await;
    let user_entry = get_user_entry(&mut data, user_id);

    if user_entry.expenses.is_empty() {
        bot.send_message(msg.chat.id, "Вы пока не записали ни одну трату").await?;
        return Ok(());
    }

    if user_entry.expenses.len() > MAX_ITEMS_IN_MESSAGE {
        bot.send_message(msg.chat.id,
            format!("Показываем {} из {} ваших трат", MAX_ITEMS_IN_MESSAGE, user_entry.expenses.len()))
            .await?;
    }

    let mut message = String::from("Ваши траты:\n\n");

    for expense in user_entry.expenses.iter().take(MAX_ITEMS_IN_MESSAGE) {
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

pub async fn show_total_expenses(
    bot: Bot,
    msg: Message,
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    let user_id = msg.from.as_ref().unwrap().id;
    let mut data = user_data.lock().await;
    let user_entry = get_user_entry(&mut data, user_id);

    if user_entry.expenses.is_empty() {
        bot.send_message(msg.chat.id, "Вы пока не записали ни одну трату").await?;
        return Ok(());
    }

    let total: f64 = user_entry.expenses.iter().map(|e| e.amount).sum();

    bot.send_message(msg.chat.id, format!("Общая сумма трат: {:.2}", total)).await?;
    Ok(())
}

pub async fn show_expenses_by_category(
    bot: Bot,
    msg: Message,
    user_data: Arc<Mutex<HashMap<UserId, UserData>>>
) -> HandlerResult {
    let user_id = msg.from.as_ref().unwrap().id;
    let mut data = user_data.lock().await;
    let user_entry = get_user_entry(&mut data, user_id);

    if user_entry.expenses.is_empty() {
        bot.send_message(msg.chat.id, "Вы пока не записали ни одну трату").await?;
        return Ok(());
    }

    let mut category_totals: HashMap<String, f64> = HashMap::new();
    for expense in &user_entry.expenses {
        *category_totals.entry(expense.category.clone()).or_default() += expense.amount;
    }

    if user_entry.categories.len() > MAX_ITEMS_IN_MESSAGE {
        bot.send_message(msg.chat.id,
            format!("Показываем {} из {} ваших категорий", MAX_ITEMS_IN_MESSAGE, user_entry.categories.len()))
            .await?;
    }

    let mut message = String::from("Траты по категориям: \n\n");
    for (category, total) in category_totals.iter().take(MAX_ITEMS_IN_MESSAGE) {
        message.push_str(&format!("{}: {:.2}\n", category, total));
    }

    bot.send_message(msg.chat.id, message).await?;
    Ok(())
}
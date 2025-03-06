use crate::*;

pub type MyDialogue = Dialogue<State, InMemStorage<State>>;
pub type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Default,
    AddExpense,
    SelectCategory {
        pending_expense: (String, f64),
    },
    ConfirmAddExpense {
        pending_expense: (String, f64),
        category: String,
    },
    AddCategory,
    ConfirmAddCategory {
        category: String,
    },
    DeleteCategory,
    ConfirmDeleteCategory {
        category: String,
    },
    CleanupExpenses,
    ConfirmCleanupExpenses,
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Expense {
    pub description: String,
    pub amount: f64,
    pub category: String,
    #[serde_as(as = "serde_with::TimestampSecondsWithFrac<String>")]
    pub date: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UserData {
    pub expenses: Vec<Expense>,
    pub categories: Vec<String>,
    pub requested_clear: bool,
    pub pending_expense: Option<(String, f64)>,
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Доступные команды")]
pub enum Command {
    #[command(description = "Показать это сообщение")]
    Help,
    #[command(description = "Показать приветственное сообщение")]
    Start,
    #[command(description = "Добавить трату")]
    AddExpense,
    // #[command(description = "Удалить трату")]
    // DeleteExpense,
    #[command(description = "Добавить категорию")]
    AddNewCategory,
    #[command(description = "Удалить категорию")]
    DeleteCategory,
    #[command(description = "Удалить все траты")]
    ClearAllExpenses,
    #[command(description = "Вывести список всех трат")]
    AllExpenses,
    #[command(description = "Вывести сумму трат")]
    TotalExpenses,
    #[command(description = "Вывести сумму трат по категориям")]
    ExpensesByCategory,
}
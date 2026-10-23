// TxnContext 没有 transaction() —— 嵌套事务是编译错误(设计 7.5)。
use app_data::TxnContext;

fn nested<'a>(tx: &'a TxnContext<'a>) {
    tx.transaction(|_t| Box::pin(async { Ok::<(), app_data::DataError>(()) }));
}

fn main() {}

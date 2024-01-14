// atomicalsir
use super::*;

#[tokio::test]
async fn get_by_ticker_should_work() {
	tracing_subscriber::fmt::init();

	ElectrumXBuilder::default().build().unwrap().get_by_ticker("quark").await.unwrap();
}

#[tokio::test]
async fn get_ft_info_should_work() {
	tracing_subscriber::fmt::init();

	let e = ElectrumXBuilder::default().build().unwrap();

	e.get_ft_info(e.get_by_ticker("quark").await.unwrap().atomical_id).await.unwrap();
}

#[tokio::test]
async fn get_unspent_address_should_work() {
	tracing_subscriber::fmt::init();

	ElectrumXBuilder::default()
		.build()
		.unwrap()
		.get_unspent_address("bc1pqkq0rg5yjrx6u08nhmc652s33g96jmdz4gjp9d46ew6ahun7xuvqaerzsp")
		.await
		.unwrap();
}

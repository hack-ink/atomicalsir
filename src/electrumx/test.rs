// atomicalsir
use super::*;

#[tokio::test]
async fn get_by_ticker_should_work() {
	let _ = tracing_subscriber::fmt::try_init();

	ElectrumXBuilder::testnet().build().unwrap().get_by_ticker("quark").await.unwrap();
}

#[tokio::test]
async fn get_ft_info_should_work() {
	let _ = tracing_subscriber::fmt::try_init();

	let e = ElectrumXBuilder::testnet().build().unwrap();

	e.get_ft_info(e.get_by_ticker("quark").await.unwrap().atomical_id).await.unwrap();
}

#[tokio::test]
async fn get_unspent_address_should_work() {
	let _ = tracing_subscriber::fmt::try_init();

	ElectrumXBuilder::testnet()
		.build()
		.unwrap()
		.get_unspent_address("tb1pemen3j4wvlryktkqsew8ext7wnsgqhmuzl7267rm3xk0th3gh04qr9wcec")
		.await
		.unwrap();
}

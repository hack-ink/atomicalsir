// std
use std::future::Future;
// crates.io
use tokio::runtime::Runtime;
// atomicals-electrumx
use crate::*;

fn test<F, Fut>(f: F)
where
	F: FnOnce(ElectrumX) -> Fut,
	Fut: Future<Output = ()>,
{
	let _ = tracing_subscriber::fmt::try_init();
	let e = ElectrumXBuilder::testnet().build().unwrap();

	Runtime::new().unwrap().block_on(f(e));
}

#[test]
fn get_by_ticker_should_work() {
	test(|e| async move {
		e.get_by_ticker("quark").await.unwrap();
	});
}

#[test]
fn get_ft_info_should_work() {
	test(|e| async move {
		e.get_ft_info(e.get_by_ticker("quark").await.unwrap().atomical_id).await.unwrap();
	});
}

#[test]
fn get_unspent_address_should_work() {
	test(|e| async move {
		e.get_unspent_address("tb1pemen3j4wvlryktkqsew8ext7wnsgqhmuzl7267rm3xk0th3gh04qr9wcec")
			.await
			.unwrap();
	});
}

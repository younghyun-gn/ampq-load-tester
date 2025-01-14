use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_lite::stream::StreamExt;
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};
use tokio::time::sleep;
use tracing::info;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR")
        .unwrap_or_else(|_| "amqp://test_admin:test_pw@52.231.104.6:5672".into());
    let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;

    info!("CONNECTED");

    let channel_a = conn.create_channel().await?;
    // let channel_b = conn.create_channel().await?;

    let queue = channel_a
        .queue_declare(
            "test-rabbitmq-queue",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!(?queue, "Declared queue");

    let published = std::sync::Arc::new(AtomicU64::new(0));

    tokio::spawn({
        let published = published.clone();
        async move {
            let mut previous_total = 0;
            loop {
                sleep(Duration::from_secs(1)).await;
                let start = tokio::time::Instant::now();
                let current_total = published.load(Ordering::Relaxed);
                let send_rate = current_total - previous_total;
                info!(
                    "Send rate: {} messages/sec, Total sent: {}",
                    send_rate, current_total
                );
                previous_total = current_total;
                info!("Time elapsed: {:?}", start.elapsed());
            }
        }
    });

    let mut consumer = channel_a
        .basic_consume(
            "test-rabbitmq-queue",
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    tokio::spawn(async move {
        info!("will consume");
        while let Some(delivery) = consumer.next().await {
            let delivery = delivery.expect("error in consumer");
            // println!("Received message: {:?}", delivery.data);
            delivery.ack(BasicAckOptions::default()).await.expect("ack");
        }
    });

    loop {
        let payload = {
            let msg_size: usize = (rand::random::<usize>() % 256).min(32);
            let mut payload = vec![0u8; msg_size];
            for byte in &mut payload {
                *byte = rand::random::<u8>();
            }
            payload
        };

        let confirm = channel_a
            .basic_publish(
                "",
                "test-rabbitmq-queue",
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default(),
            )
            .await?
            .await?;
        assert_eq!(confirm, Confirmation::NotRequested);
        published.fetch_add(1, Ordering::Relaxed);
    }
}

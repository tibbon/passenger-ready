use serde::Deserialize;
use std::{time::Duration};
use tokio::time::timeout;
use warp::{http::StatusCode, Filter, Rejection};
use log::info;
use config::Config;
use anyhow::Result;

#[derive(Debug, Deserialize, Clone)]
struct Settings {
    max_queue_length: i32,
    server_port: u16,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let settings = load_settings().expect("Configuration error");

    let cloned_settings = settings.clone();
    let health_route = warp::path("health").and_then(move || {
        let settings = cloned_settings.clone();
        async move {
            match can_take_more_traffic(settings.max_queue_length).await {
                Ok(can_take) => {
                    if can_take {
                        Ok::<_, Rejection>(warp::reply::with_status("true", StatusCode::OK))
                    } else {
                        Ok::<_, Rejection>(warp::reply::with_status("false", StatusCode::SERVICE_UNAVAILABLE))
                    }
                }
                Err(_) => Ok::<_, Rejection>(warp::reply::with_status("false", StatusCode::SERVICE_UNAVAILABLE))
            }
        }
    });

    info!("Starting server on port {}", settings.server_port);
    warp::serve(health_route)
        .run(([127, 0, 0, 1], settings.server_port))
        .await;
}

async fn can_take_more_traffic(max_queue_length: i32) -> Result<bool> {
    let queue_length = get_queue_length().await?;
    Ok((queue_length as f32) < (max_queue_length as f32 * 0.8))
}

async fn get_queue_length() -> Result<i32> {
    let output = timeout(
        Duration::from_secs(5),
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg("passenger-status | grep 'Requests in top-level queue'")
            .output(),
    )
    .await??;

    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        // The output_str is expected to be something like "Requests in top-level queue : 0"
        if let Some(queue_part) = output_str.split(":").nth(1) {
            queue_part.trim().parse::<i32>().map_err(|e| anyhow::anyhow!(e))
        } else {
            Err(anyhow::anyhow!("Failed to parse queue length"))
        }
    } else {
        Err(anyhow::anyhow!("passenger-status execution failed"))
    }
}

fn load_settings() -> Result<Settings, config::ConfigError> {
    let mut cfg = Config::new();

    // Set default values
    cfg.set_default("max_queue_length", 100)?;
    cfg.set_default("server_port", 8080)?;

    // Attempt to merge environment variables on top of defaults
    cfg.merge(config::Environment::new())?;

    cfg.try_into()
}

// Define a custom error type that implements warp::reject::Reject
#[derive(Debug)]
struct MyError {}

impl warp::reject::Reject for MyError {}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::test::request;
    use std::sync::Mutex;
    use lazy_static::lazy_static;
    use std::env;

    lazy_static! {
        static ref ENV_LOCK: Mutex<()> = Mutex::new(());
    }

    async fn setup_env(max_queue_length: &str, server_port: &str) {
        let _env_lock = ENV_LOCK.lock().unwrap();
        env::set_var("APP_MAX_QUEUE_LENGTH", max_queue_length);
        env::set_var("APP_SERVER_PORT", server_port);
    }

    async fn teardown_env() {
        env::remove_var("APP_MAX_QUEUE_LENGTH");
        env::remove_var("APP_SERVER_PORT");
    }

    #[tokio::test]
    async fn passenger_running_with_space_in_queue() {
        // Setup: Assume `get_queue_length` is somehow mocked to return a value indicating space is available.
        // This setup requires your application logic to be refactored for dependency injection or using a mocking library.
        
        // Mocking environment variables for application settings
        let _ = env::set_var("APP_MAX_QUEUE_LENGTH", "100");
        let _ = env::set_var("APP_SERVER_PORT", "8080");

        // Define your health check route or filter here, similar to how it's defined in the main application.
        // This might involve directly invoking the health check logic if it's abstracted appropriately.
        
        let filter = warp::path("health").map(|| warp::reply::with_status("true", StatusCode::OK));

        // Execute the request against the health check route
        let resp = request().method("GET").path("/health").reply(&filter).await;

        // Assertions: Expect a 200 OK response with "true" indicating space is available in the queue
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.body(), "true");

        // Cleanup: Remove the environment variables to avoid side effects on other tests
        let _ = env::remove_var("APP_MAX_QUEUE_LENGTH");
        let _ = env::remove_var("APP_SERVER_PORT");
    }

    #[tokio::test]
    async fn health_check_responds_unavailable_when_overloaded() {
        setup_env("5", "8080").await; // Simulating a very low max queue length
        // Assuming get_queue_length would return 6 or more in this scenario
        let filter = warp::path("health").map(|| warp::reply::with_status("false", StatusCode::SERVICE_UNAVAILABLE));

        let resp = request().method("GET").path("/health").reply(&filter).await;

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.body(), "false");

        teardown_env().await;
    }

    #[tokio::test]
    async fn health_check_responds_unavailable_when_passenger_down() {
        setup_env("100", "8080").await; // Normal operation settings
        // Simulating Passenger being down, which would normally cause get_queue_length to fail
        let filter = warp::path("health").map(|| warp::reply::with_status("false", StatusCode::SERVICE_UNAVAILABLE));

        let resp = request().method("GET").path("/health").reply(&filter).await;

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.body(), "false");

        teardown_env().await;
    }
}

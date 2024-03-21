use std::{env, process::Command};
use warp::Filter;

#[tokio::main]
async fn main() {
    let health_route = warp::path("health").map(|| {
        if can_take_more_traffic(&RealPassengerStatus) {
            warp::reply::json(&"true")
        } else {
            warp::reply::json(&"false")
        }
    });

    warp::serve(health_route)
        .run(([127, 0, 0, 1], 8080))
        .await;
}

fn can_take_more_traffic(status: &dyn PassengerStatus) -> bool {
    let queue_length = status.get_queue_length().unwrap_or_default();
    let max_queue_length = get_max_queue_length().unwrap_or(100);

    (queue_length as f32) < (max_queue_length as f32 * 0.8)
}

fn get_max_queue_length() -> Option<i32> {
    env::var("MAX_QUEUE_LENGTH")
        .ok()
        .and_then(|val| val.parse::<i32>().ok())
}

trait PassengerStatus {
    fn get_queue_length(&self) -> Option<i32>;
}

struct RealPassengerStatus;

impl PassengerStatus for RealPassengerStatus {
    fn get_queue_length(&self) -> Option<i32> {
        let output = Command::new("sh")
            .arg("-c")
            .arg("passenger-status | grep 'Requests in queue' | awk '{print $5}'")
            .output()
            .expect("failed to execute process");

        let output_str = String::from_utf8_lossy(&output.stdout);
        output_str.trim().parse::<i32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use warp::http::StatusCode;
    use warp::test::request;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref ENV_MUTEX: Mutex<()> = Mutex::new(());
    }

    #[derive(Clone)]
    struct MockPassengerStatus;

    impl PassengerStatus for MockPassengerStatus {
        fn get_queue_length(&self) -> Option<i32> {
            Some(42) // Mocked value
        }
    }

    #[tokio::test]
    async fn test_can_take_more_traffic_with_mock() {
        assert!(can_take_more_traffic(&MockPassengerStatus));
    }

    #[tokio::test]
    async fn test_health_route_with_mock() {
        let mock_status: MockPassengerStatus = MockPassengerStatus {};
        let filter = warp::path("health").map(move || {
            if can_take_more_traffic(&mock_status) {
                warp::reply::with_status("true", StatusCode::OK)
            } else {
                warp::reply::with_status("false", StatusCode::OK)
            }
        });

        let response = request().method("GET").path("/health").reply(&filter).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body(), "true");
    }

    #[test]
    fn test_get_max_queue_length_not_set() {
        let _lock = ENV_MUTEX.lock().unwrap();
        env::remove_var("MAX_QUEUE_LENGTH");
        assert_eq!(get_max_queue_length(), None);
    }

    #[test]
    fn test_get_max_queue_length_set_and_valid() {
        let _lock = ENV_MUTEX.lock().unwrap();
        env::set_var("MAX_QUEUE_LENGTH", "150");
        assert_eq!(get_max_queue_length(), Some(150));
        env::remove_var("MAX_QUEUE_LENGTH");
    }

    #[test]
    fn test_get_max_queue_length_set_and_invalid() {
        let _lock = ENV_MUTEX.lock().unwrap();
        env::set_var("MAX_QUEUE_LENGTH", "invalid_int");
        assert_eq!(get_max_queue_length(), None);
        env::remove_var("MAX_QUEUE_LENGTH");
    }
}

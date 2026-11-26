//! The search thread of ADR 0002 and `docs/core.md`: one worker thread, a
//! new `CancelToken` for each query, and the results back to the main loop.

use std::sync::mpsc;
use std::thread;

use petlauncher_core::{CancelToken, Results};

/// The search function. It runs on the worker thread.
pub type SearchFn = dyn Fn(&str, &CancelToken) -> Results + Send;

pub struct SearchWorker {
    requests: mpsc::Sender<(String, CancelToken)>,
    current: CancelToken,
}

impl SearchWorker {
    /// Starts the thread. It sends the results of each search that was not
    /// cancelled to `results`. The thread stops when the worker is dropped.
    pub fn start(search: Box<SearchFn>, results: async_channel::Sender<Results>) -> Self {
        let (requests, received) = mpsc::channel();
        thread::Builder::new()
            .name("petlauncher-search".to_owned())
            .spawn(move || run(&received, &*search, &results))
            .expect("cannot start the search thread");
        Self {
            requests,
            current: CancelToken::new(),
        }
    }

    /// Cancels the search of the last query and searches `query`.
    pub fn search(&mut self, query: &str) {
        self.current.cancel();
        self.current = CancelToken::new();
        // The thread stops only when this worker is dropped, so the send
        // cannot fail.
        let _ = self.requests.send((query.to_owned(), self.current.clone()));
    }
}

impl Drop for SearchWorker {
    fn drop(&mut self) {
        self.current.cancel();
    }
}

fn run(
    received: &mpsc::Receiver<(String, CancelToken)>,
    search: &SearchFn,
    results: &async_channel::Sender<Results>,
) {
    while let Ok(mut request) = received.recv() {
        // Only the newest query is of interest.
        while let Ok(newer) = received.try_recv() {
            request = newer;
        }
        let (query, cancel) = request;
        if cancel.is_cancelled() {
            continue;
        }
        let found = search(&query, &cancel);
        if !cancel.is_cancelled() && results.send_blocking(found).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn results_for(query: &str) -> Results {
        Results {
            query: query.to_owned(),
            ..Results::default()
        }
    }

    #[test]
    fn sends_the_results_of_a_query_to_the_main_loop() {
        let (sender, receiver) = async_channel::unbounded();
        let mut worker = SearchWorker::start(Box::new(|query, _| results_for(query)), sender);

        worker.search("lock");

        let results = receiver.recv_blocking().unwrap();
        assert_eq!(results.query, "lock");
    }

    #[test]
    fn a_new_query_cancels_the_running_search() {
        let (sender, receiver) = async_channel::unbounded();
        let (started_sender, started) = mpsc::channel();
        let search = move |query: &str, cancel: &CancelToken| {
            if query == "slow" {
                started_sender.send(()).unwrap();
                while !cancel.is_cancelled() {
                    thread::sleep(Duration::from_millis(1));
                }
            }
            results_for(query)
        };
        let mut worker = SearchWorker::start(Box::new(search), sender);

        worker.search("slow");
        started.recv().unwrap();
        worker.search("fast");

        let results = receiver.recv_blocking().unwrap();
        assert_eq!(results.query, "fast");
        assert!(receiver.is_empty());
    }

    #[test]
    fn searches_only_the_newest_of_the_waiting_queries() {
        let (sender, receiver) = async_channel::unbounded();
        let searched = Arc::new(Mutex::new(Vec::new()));
        let (started_sender, started) = mpsc::channel();
        let (release_sender, release) = mpsc::channel::<()>();
        let log = Arc::clone(&searched);
        let search = move |query: &str, _: &CancelToken| {
            if query == "first" {
                started_sender.send(()).unwrap();
                release.recv().unwrap();
            }
            log.lock().unwrap().push(query.to_owned());
            results_for(query)
        };
        let mut worker = SearchWorker::start(Box::new(search), sender);

        worker.search("first");
        started.recv().unwrap();
        worker.search("s");
        worker.search("se");
        worker.search("sec");
        release_sender.send(()).unwrap();

        let results = receiver.recv_blocking().unwrap();
        assert_eq!(results.query, "sec");
        assert_eq!(*searched.lock().unwrap(), ["first", "sec"]);
    }

    #[test]
    fn the_thread_stops_when_the_worker_is_dropped() {
        let (sender, receiver) = async_channel::unbounded();
        let worker = SearchWorker::start(Box::new(|query, _| results_for(query)), sender);

        drop(worker);

        assert!(receiver.recv_blocking().is_err());
    }

    #[test]
    fn the_thread_stops_when_the_main_loop_is_gone() {
        let (sender, receiver) = async_channel::unbounded();
        let (done_sender, done) = mpsc::channel();
        let search = move |query: &str, _: &CancelToken| {
            let _ = done_sender.send(());
            results_for(query)
        };
        let mut worker = SearchWorker::start(Box::new(search), sender);
        drop(receiver);

        worker.search("a");
        done.recv().unwrap();

        // The thread drops the search function, and its sender, when it stops.
        assert_eq!(
            done.recv_timeout(Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
    }
}

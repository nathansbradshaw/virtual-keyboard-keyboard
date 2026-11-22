use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Evaluate an algorithm with a blocking budget. Such futures have no
/// suspension points; accidentally passing a cooperative browser job fails
/// immediately instead of blocking the browser's event loop.
pub fn complete_immediately<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("a blocking work budget unexpectedly suspended"),
    }
}
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[derive(Debug, Clone, Default)]
pub struct LoadGeneration(Arc<AtomicU64>);

#[derive(Debug, Clone)]
pub struct LoadTicket {
    generation: Arc<AtomicU64>,
    id: u64,
}

impl LoadGeneration {
    pub fn begin(&self) -> LoadTicket {
        LoadTicket {
            generation: self.0.clone(),
            id: self.0.fetch_add(1, Ordering::Relaxed).wrapping_add(1),
        }
    }
}

impl LoadTicket {
    pub fn is_current(&self) -> bool {
        self.generation.load(Ordering::Relaxed) == self.id
    }
}

/// Browser work gets regular event-loop turns. Native/synchronous callers
/// use the same algorithms without scheduling browser callbacks.
pub struct WorkBudget {
    ticket: Option<LoadTicket>,
    cooperative: bool,
    operations: usize,
    last_yield: Instant,
}

impl WorkBudget {
    pub fn blocking() -> Self {
        Self {
            ticket: None,
            cooperative: false,
            operations: 0,
            last_yield: Instant::now(),
        }
    }

    pub fn loading(ticket: LoadTicket) -> Self {
        Self {
            ticket: Some(ticket),
            cooperative: true,
            ..Self::blocking()
        }
    }

    pub fn check_current(&self) -> Result<(), String> {
        if self
            .ticket
            .as_ref()
            .is_some_and(|ticket| !ticket.is_current())
        {
            Err("MIDI loading cancelled".into())
        } else {
            Ok(())
        }
    }

    pub async fn checkpoint(&mut self) -> Result<(), String> {
        self.operations = self.operations.wrapping_add(1);
        if self.operations & 1023 != 0 {
            return Ok(());
        }
        self.check_current()?;
        if self.cooperative && self.last_yield.elapsed() >= std::time::Duration::from_millis(4) {
            yield_to_browser().await?;
            self.last_yield = Instant::now();
            self.check_current()?;
        }
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
async fn yield_to_browser() -> Result<(), String> {
    // Posted messages yield a browser task without nested setTimeout's 4 ms
    // minimum delay. All parsing and preparation remain Rust code.
    let channel = web_sys::MessageChannel::new().map_err(|_| "Could not schedule MIDI loading")?;
    let receiver = channel.port1();
    let sender = channel.port2();
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        receiver.set_onmessage(Some(&resolve));
    });
    sender
        .post_message(&wasm_bindgen::JsValue::NULL)
        .map_err(|_| "Could not continue MIDI loading")?;
    let result = wasm_bindgen_futures::JsFuture::from(promise).await;
    receiver.set_onmessage(None);
    receiver.close();
    sender.close();
    result
        .map(|_| ())
        .map_err(|_| "Could not continue MIDI loading".into())
}

#[cfg(not(target_arch = "wasm32"))]
async fn yield_to_browser() -> Result<(), String> {
    Ok(())
}

/// Stable merge sort with bounded chunks and cooperative merge passes.
pub async fn sort_by_key<T: Copy, K: Ord>(
    values: &mut Vec<T>,
    key: impl Fn(&T) -> K,
    budget: &mut WorkBudget,
) -> Result<(), String> {
    const CHUNK: usize = 1024;
    // Single-track files and ordinary note pairs often arrive already in
    // order. Keep that common case linear and avoid a second full buffer.
    let mut ordered = true;
    for i in 1..values.len() {
        if key(&values[i - 1]) > key(&values[i]) {
            ordered = false;
            break;
        }
        budget.checkpoint().await?;
    }
    if ordered {
        return budget.check_current();
    }
    for chunk in values.chunks_mut(CHUNK) {
        chunk.sort_by_key(&key);
        for _ in 0..chunk.len() {
            budget.checkpoint().await?;
        }
    }
    if values.len() <= CHUNK {
        return budget.check_current();
    }
    let mut source = std::mem::take(values);
    let mut destination = Vec::with_capacity(source.len());
    let mut width = CHUNK;
    while width < source.len() {
        for start in (0..source.len()).step_by(width.saturating_mul(2)) {
            let middle = (start + width).min(source.len());
            let end = middle.saturating_add(width).min(source.len());
            let (mut left, mut right) = (start, middle);
            while left < middle || right < end {
                let index = if right == end
                    || (left < middle && key(&source[left]) <= key(&source[right]))
                {
                    let index = left;
                    left += 1;
                    index
                } else {
                    let index = right;
                    right += 1;
                    index
                };
                destination.push(source[index]);
                budget.checkpoint().await?;
            }
        }
        std::mem::swap(&mut source, &mut destination);
        destination.clear();
        width = width.saturating_mul(2);
    }
    *values = source;
    budget.check_current()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_requests_invalidate_old_results_and_cancel_work() {
        let generation = LoadGeneration::default();
        let old = generation.begin();
        let budget = WorkBudget::loading(old.clone());
        assert!(old.is_current());
        let current = generation.begin();
        assert!(!old.is_current());
        assert!(current.is_current());
        assert!(budget.check_current().is_err());
    }

    #[test]
    fn cooperative_sort_preserves_equal_key_order_across_chunks() {
        let mut values: Vec<_> = (0..10_003).map(|i| ((i * 37) % 23, i)).collect();
        let mut expected = values.clone();
        expected.sort_by_key(|v| v.0);
        complete_immediately(sort_by_key(
            &mut values,
            |v| v.0,
            &mut WorkBudget::blocking(),
        ))
        .unwrap();
        assert_eq!(values, expected);
    }
}

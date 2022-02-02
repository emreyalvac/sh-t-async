use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[derive(Default, Debug)]
struct Parker(Mutex<bool>, Condvar);

struct MyWaker {
    parker: Arc<Parker>,
}

impl Parker {
    fn park(&self) {
        let mut resumable = self.0.lock().unwrap();
        while !*resumable {
            resumable = self.1.wait(resumable).unwrap();
        }
        *resumable = false;
    }

    fn unpark(&self) {
        *self.0.lock().unwrap() = true;
        self.1.notify_one();
    }
}

fn my_waker_unpark(my_waker: &MyWaker) {
    my_waker.parker.unpark();
}

fn raw_waker(my_waker: *const MyWaker) -> RawWaker {
    let v_table = unsafe {
        &RawWakerVTable::new(
            |s| raw_waker(&*(s as *const MyWaker)),
            |s| my_waker_unpark(&*(s as *const MyWaker)),
            |s| my_waker_unpark(&*(s as *const MyWaker)),
            |s| drop(s),
        )
    };

    RawWaker::new(my_waker as *const (), v_table)
}

fn waker(my_waker: *const MyWaker) -> Waker {
    unsafe { Waker::from_raw(raw_waker(my_waker)) }
}

fn block_on<F: Future<Output = ()>>(mut f: F) -> F::Output {
    let parker = Arc::new(Parker::default());
    let my_waker = Arc::new(MyWaker {
        parker: parker.clone(),
    });
    let waker = waker(Arc::into_raw(my_waker));
    let mut cx = Context::from_waker(&waker);

    let mut future = unsafe { Pin::new_unchecked(&mut f) };

    loop {
        match Future::poll(future.as_mut(), &mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => parker.park(),
        };
    }
}

fn main() {
    let a = async {
        println!("async");
    };

    block_on(async {
        a.await;
    });
}

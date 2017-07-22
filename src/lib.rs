use std::sync::{Mutex, Condvar};
use std::ops::{Deref, Drop};
use std::mem;

pub struct Blocker<'a, T: 'a> {
    data: &'a T,
    ref_count: usize,
    mutex: Mutex<()>,
    cond: Condvar,
}
impl<'a, T> Blocker<'a, T> {
    pub fn new(data: &'a T) -> Blocker<'a, T> {
        Blocker {
            data,
            ref_count: 0,
            mutex: Mutex::new(()),
            cond: Condvar::new(),
        }
    }
    pub fn get(&self) -> Blockref<T> {
        unsafe {
            Blockref::new(
                self.data,
                mem::transmute(&self.ref_count),
                &self.mutex,
                &self.cond,
            )
        }
    }
}
impl<'a, T> Drop for Blocker<'a, T> {
    fn drop(&mut self) {
        let mut lock = self.mutex.lock().unwrap();
        loop {
            lock = self.cond.wait(lock).unwrap();
            if self.ref_count == 0 {
                break;
            }
        }
    }
}

pub struct Blockref<T> {
    data: *const T,
    ref_count: *mut usize,
    mutex: *const Mutex<()>,
    cond: *const Condvar,
}
impl<T> Blockref<T> {
    fn new(
        data: *const T,
        ref_count: *mut usize,
        mutex: *const Mutex<()>,
        cond: *const Condvar,
    ) -> Blockref<T> {
        unsafe {
            let _lock = (*mutex).lock().unwrap();
            *ref_count += 1;
        }
        Blockref {
            data,
            ref_count,
            mutex,
            cond,
        }
    }
}
impl<T> Drop for Blockref<T> {
    fn drop(&mut self) {
        unsafe {
            let _lock = (*self.mutex).lock().unwrap();
            *self.ref_count -= 1;
            (*self.cond).notify_one();
        }
    }
}
impl<T> Deref for Blockref<T> {
    type Target = T;
    fn deref(&self) -> &T {
        return unsafe { mem::transmute(self.data) };
    }
}
unsafe impl<T> Sync for Blockref<T> {}
unsafe impl<T> Send for Blockref<T> {}

#[test]
fn test_basic() {
    use std::thread;
    let x = 5;
    let y = Blocker::new(&x);
    let z = y.get();
    thread::spawn(move || {
        thread::sleep_ms(1000);
        println!("{}", *z);
    });
}

#[derive(Debug)]
struct Test {
    x: usize,
}
impl Drop for Test {
    fn drop(&mut self) {
        self.x = 0;
    }
}
#[test]
fn test_multi_explicit_drop() {
    use std::thread;
    let x = Test { x: 5 };
    let y = Blocker::new(&x);
    let z = y.get();
    let w = y.get();
    let v = y.get();
    thread::spawn(move || {
        thread::sleep_ms(1000);
        println!("z{:?}", *z);
    });
    thread::spawn(move || {
        thread::sleep_ms(500);
        println!("w{:?}", *w);
    });
    thread::spawn(move || {
        thread::sleep_ms(100);
        println!("v{:?}", *v);
    });
    drop(y);
}
#[test]
fn test_multi() {
    use std::thread;
    let x = Test { x: 5 };
    let y = Blocker::new(&x);
    let z = y.get();
    let w = y.get();
    let v = y.get();
    thread::spawn(move || {
        thread::sleep_ms(1000);
        println!("z{:?}", *z);
    });
    thread::spawn(move || {
        thread::sleep_ms(500);
        println!("w{:?}", *w);
    });
    thread::spawn(move || {
        thread::sleep_ms(100);
        println!("v{:?}", *v);
    });
}
#[test]
fn test_race() {
    use std::thread;
    use std::sync::Barrier;
    use std::sync::Arc;
    for i in 0..1000 {
        let mut x = Test { x: 5 };
        let barrier = Arc::new(Barrier::new(2));
        {
            let y = Blocker::new(&x);
            let z = y.get();
            let w = y.get();
            let b2 = barrier.clone();
            thread::spawn(move || {
                thread::sleep_ms(1);
                b2.wait();
                assert!(z.x == 5);
                drop(z);
            });
            thread::spawn(move || {
                thread::sleep_ms(1);
                barrier.wait();
                assert!(w.x == 5);
                drop(w);
            });
            drop(y);
        }
        x.x = 6;
    }
}

#![allow(dead_code, unused_variables)]

use std::{thread, time};
use rand::Rng;
use test_log::test;

use rlu::{
  rlu_dereference, rlu_reader_lock, rlu_reader_unlock,
  rlu_try_lock, rlu_thread_init, rlu_abort, RluGlobal, Rlu
};



#[derive(Copy, Clone, Debug)]
pub struct RluInt64Wrapper {
  pub obj : *mut Rlu<u64>,
  pub rlu_global : *mut RluGlobal<u64>
}


unsafe impl Send for RluInt64Wrapper {}
unsafe impl Sync for RluInt64Wrapper {}


#[test_log::test]
fn rlu_basic_spawn_threads() {
  /* Put your RLU tests here! Or add more functions below. */
  let rlu_global: *mut RluGlobal<u64> = RluGlobal::init();
  let rlu_global_obj = unsafe { &*rlu_global };

  let id = rlu_thread_init(rlu_global);
  println!("Spawned thread: {id}");

  let id1 = rlu_thread_init(rlu_global);
  println!("Spawned thread: {id1}");
  
  let id2 = rlu_thread_init(rlu_global);
  println!("Spawned thread: {id2}");


  assert_eq!(id, 0);
  assert_eq!(id1, 1);
  assert_eq!(id2, 2);

  
}

#[test_log::test]
fn rlu_multiple_threads_read_only() {
  let rlu_global : *mut RluGlobal<u64> = RluGlobal::init();
  let rlu_global_obj = unsafe { & *rlu_global };

  let test_val = 2;
  let wrapped_int64_obj = RluInt64Wrapper { // need wrapper for unsafe send and sync
    obj : Box::into_raw(Box::new(rlu_global_obj.alloc(test_val))),
    rlu_global: rlu_global,
  };


    let reader1 = thread::spawn(move || unsafe {
    let wrapped_int64_obj = wrapped_int64_obj; // needed in 2021 version of Rust
    let obj = wrapped_int64_obj.obj;
    let rglobal = wrapped_int64_obj.rlu_global;

    let id1 = rlu_thread_init(rglobal);
    println!("Spawned Reader RLU thread: {id1}");

    rlu_reader_lock(rglobal, id1);

    thread::sleep(time::Duration::from_millis(100));

    let after  = rlu_dereference(rglobal, id1, obj);
    assert_eq!(test_val, *after);
    rlu_reader_unlock(rglobal, id1);

  });


  let reader2 = thread::spawn(move || unsafe {
    let wrapped_int64_obj = wrapped_int64_obj;
    let obj = wrapped_int64_obj.obj;
    let rglobal = wrapped_int64_obj.rlu_global;

    let id2 = rlu_thread_init(rglobal);

    println!("Spawned RLU thread: {id2}");
    rlu_reader_lock(rglobal, id2);
    thread::sleep(time::Duration::from_millis(100));

    let after  = rlu_dereference(rglobal, id2, obj);
    assert_eq!(test_val, *after);
    rlu_reader_unlock(rglobal, id2);

  });

  reader1.join().unwrap();
  reader2.join().unwrap();
  
} 


#[test_log::test]
fn rlu_single_read_single_writer() {
  let rlu_global : *mut RluGlobal<u64> = RluGlobal::init();
  let rlu_global_obj = unsafe { & *rlu_global };

  let wrapped_int64_obj = RluInt64Wrapper { // need wrapper for unsafe send and sync
    obj : Box::into_raw(Box::new(rlu_global_obj.alloc(2))),
    rlu_global: rlu_global,
  };


  let reader = thread::spawn(move || unsafe {
      let wrapped_int64_obj = wrapped_int64_obj;
      let obj = wrapped_int64_obj.obj;
      let rglobal = wrapped_int64_obj.rlu_global;
  
      let id1 = rlu_thread_init(rglobal);
      println!("Spawned Reader RLU thread: {id1}");
      
      /* will hold lock value will not change */
      rlu_reader_lock(rglobal, id1);
      let val = rlu_dereference(rglobal, id1, obj);
      let before = *val;
      thread::sleep(time::Duration::from_millis(200));
      assert_eq!(*val, before);
      rlu_reader_unlock(rglobal, id1);


      /* value will change because it happens after 200 millis */
      rlu_reader_lock(rglobal, id1);
      let obj2 = rlu_dereference(rglobal, id1, obj);
      assert_eq!(*obj2, 3);
      rlu_reader_unlock(rglobal, id1);
    

  });

  let reader1 = thread::spawn(move || unsafe {
      let wrapped_int64_obj = wrapped_int64_obj;
      let obj = wrapped_int64_obj.obj;
      let rglobal = wrapped_int64_obj.rlu_global;

      let id1 = rlu_thread_init(rglobal);
      println!("Spawned Reader RLU thread: {id1}");
      
      /* will hold lock value will not change */
      rlu_reader_lock(rglobal, id1);
      let val = rlu_dereference(rglobal, id1, obj);
      let before = *val;
      thread::sleep(time::Duration::from_millis(200));
      assert_eq!(*val, before);
      rlu_reader_unlock(rglobal, id1);


      /* value will change because it happens after 200 millis */
      rlu_reader_lock(rglobal, id1);
      let obj2 = rlu_dereference(rglobal, id1, obj);
      assert_eq!(*obj2, 3);
      rlu_reader_unlock(rglobal, id1);
    

  });

  let writer = thread::spawn(move || unsafe {
      let wrapped_int64_obj = wrapped_int64_obj;
      let obj = wrapped_int64_obj.obj;
      let rglobal = wrapped_int64_obj.rlu_global;
  
      let id2 = rlu_thread_init(rglobal);
      println!("Spawned Writer RLU thread: {id2}");
  
      //rlu_reader_lock(rglobal, id2);
  
      let obj2 = rlu_dereference(rglobal, id2, obj);
  
      assert_eq!(*obj2, 2);

      let obj3 = rlu_try_lock(rglobal, id2, obj).unwrap();
      //assert!(rlu_try_lock(rglobal, id2, obj2)); // Fix: pass obj2
      
      // TODO: this is not modifying: fix required in rlu_dereference
      *obj3 += 1;
  
      //rlu_reader_unlock(rglobal, id2);
  });

  reader.join().unwrap();
  writer.join().unwrap();
  
} 

#[test_log::test]
fn rlu_hold_locks() {
  let rlu_global : *mut RluGlobal<u64> = RluGlobal::init();
  let rlu_global_obj = unsafe { & *rlu_global };

  let wrapped_int64_obj = RluInt64Wrapper { // need wrapper for unsafe send and sync
    obj : Box::into_raw(Box::new(rlu_global_obj.alloc(0))),
    rlu_global: rlu_global,
  };


  let reader = |x : u64| {
    
    thread::spawn(move || unsafe {
      let wrapped_int64_obj = wrapped_int64_obj;
      let obj = wrapped_int64_obj.obj;
      let rglobal = wrapped_int64_obj.rlu_global;
  
      let id1 = rlu_thread_init(rglobal);
      println!("Spawned Reader RLU thread: {id1}");

      for _ in 0..100{
  
        rlu_reader_lock(rglobal, id1);
    
        let val = rlu_dereference(rglobal, id1, obj);
        let before = *val;
        thread::sleep(time::Duration::from_millis(10));
    
        assert_eq!(before, *val);

        rlu_reader_unlock(rglobal, id1);
      }


      println!("Reader {} exited", x);
    })
  };


  let writer = |x : u64| {
    thread::spawn(move || unsafe {
      let wrapped_int64_obj = wrapped_int64_obj;
      let obj = wrapped_int64_obj.obj;
      let rglobal = wrapped_int64_obj.rlu_global;
  
      let id = rlu_thread_init(rglobal);
      println!("Spawned Writer RLU thread: {id}");
      
      for i in 0..1000 {

        loop {
          rlu_reader_lock(rglobal, id);
          let wobj = rlu_try_lock(rglobal, id, obj);

          match wobj {
            None => {
              rlu_abort(rglobal, id);
              continue;
            }

            Some(wobj) => {
              *wobj += 1;
              break;
            }
          }
        }

        rlu_reader_unlock(rglobal, id);
        
      }


      println!("Writer {} exited", x);
    })
  
  };
  let num_readers = 16;
  let num_writers = 2;

  let readers: Vec<_> = (0..num_readers).map(|i| reader(i)).collect();
  let writers: Vec<_> = (0..num_writers).map(|i| writer(i)).collect();

  for t in readers {
    t.join().expect("Reader panicked");
  }

  for t in writers {
    t.join().expect("Writer panicked");
  }


  // reader.join().unwrap();
  // writer.join().unwrap();

  unsafe {
    let obj = wrapped_int64_obj.obj;
    let rglobal = wrapped_int64_obj.rlu_global;

    let id = rlu_thread_init(rglobal);
    rlu_reader_lock(rglobal, id);
    let val = rlu_dereference(rglobal, id, obj);
    assert_eq!(*val, 1000 * num_writers);
    rlu_reader_unlock(rglobal, id);

  }
  
  
}


// #[test_log::test]
// fn single_read_write_hundred() {
//   for i in 0..100 {
//     println!("Iteration {i}:");
//     rlu_single_read_single_writer();
//   }
// }

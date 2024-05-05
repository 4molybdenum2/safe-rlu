use crate::concurrent_set::ConcurrentSet;
use crate::rlu::{
self, Rlu, RluGlobal, RluThreadData
};
use crate::{rlu_abort, rlu_dereference, rlu_reader_lock, rlu_reader_unlock, rlu_thread_init, rlu_try_lock};
use std::fmt::Debug;
use std::marker::{Unpin, PhantomData};
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::{mem, ptr};


pub struct RluSet<T : 'static + Clone> {
  head: Rlu<RluNode<T>>,
  thread_id: usize,
  rlu_global: *mut RluGlobal<RluNode<T>>,
}


#[derive(Debug, Clone, Copy)]
pub struct RluNode<T>{
  elem: T,
  next: *mut Rlu<RluNode<T>>,
}


// In case you need raw pointers in your RluSet, you can assert that RluSet is definitely
// Send and Sync  
unsafe impl<T : Clone> Send for RluSet<T> {}
unsafe impl<T : Clone> Sync for RluSet<T> {}

impl<T> RluSet<T> where T: PartialEq + PartialOrd + Copy + Clone + Debug + Unpin {
  pub fn new() -> RluSet<T> {

    let rlu_global : *mut RluGlobal<RluNode<T>> = RluGlobal::init();
    let rlu_global_obj = unsafe { & *rlu_global };
    let thread_id = rlu_thread_init(rlu_global);

    RluSet{
      head: rlu_global_obj.alloc(
        RluNode {
          elem: unsafe{ mem::uninitialized()},
          next: ptr::null_mut(),
        }
      ),
      thread_id: thread_id,
      rlu_global: rlu_global,
    }
  }

  pub fn to_string(&self) -> String {
    unimplemented!()
      // let mut ret = String::from("{");
      // unsafe {
      //     let mut node_ptr = (self.head).next;
      //     loop {
      //         if node_ptr.is_null() {
      //             break;
      //         } else {
      //             ret.push_str(&format!("{:?}, ", (*node_ptr).data));
      //             node_ptr = (*node_ptr).next;
      //         }
      //     }
      // }
      // ret.push('}');
      // ret
  }

}


impl<T> ConcurrentSet<T> for RluSet<T> where T: PartialEq + PartialOrd + Copy + Clone + Debug + Unpin {
  
  fn contains(&self, value: T) -> bool {
    let mut ret = false;

    rlu_reader_lock(self.rlu_global, self.thread_id);

    let mut node_ptr = &self.head as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

    let mut node = rlu_dereference(self.rlu_global, self.thread_id, node_ptr);


    let mut first_deref = true;

    loop {
          if node_ptr.is_null() {
            break;
          }
          else if first_deref {

            first_deref = false;

            let next_ptr = unsafe{ (*node).next }; 

            if next_ptr.is_null() {
              break;
            }

            node_ptr = next_ptr;

            continue;

          } else {

            node = rlu_dereference(self.rlu_global, self.thread_id, node_ptr);

            let v = unsafe{ (*node).elem};

            if v > value {
              break;
            }

            if v == value {
              ret = true;
              break;
            }
            
            let next_ptr = unsafe { (*node).next };
            // increment ptr
            node_ptr = next_ptr;
            
          }
    }

    rlu_reader_unlock(self.rlu_global, self.thread_id);
    ret
  }



  fn len(&self) -> usize {
      let mut len = 0;

      rlu_reader_lock(self.rlu_global, self.thread_id);

      let mut node_ptr = &self.head as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

      let mut node = rlu_dereference(self.rlu_global, self.thread_id, node_ptr);

      let mut first_deref = true;


      loop {
        if node_ptr.is_null() {
          break;
        } else {
          if !first_deref {
            len += 1;
          } else {
            first_deref = false;
          }

          node = rlu_dereference(self.rlu_global, self.thread_id, node_ptr);

          // increment ptr
          node_ptr = unsafe{ (*node).next };
        }
      }

      rlu_reader_unlock(self.rlu_global, self.thread_id);

      len
    }

  fn insert(&mut self, value: T) -> bool {
    let rlu_global_obj = unsafe { &*self.rlu_global };

    loop {
        rlu_reader_lock(self.rlu_global, self.thread_id);

        let mut prev_ptr = &self.head as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

        let mut prev = rlu_dereference(self.rlu_global, self.thread_id, prev_ptr); // ptr to list head


        let mut next_ptr = unsafe { (*prev).next };

        let mut next = rlu_dereference(self.rlu_global, self.thread_id, next_ptr); // ptr to next ptr to prev

        let mut matches = false;

        loop {
            if next_ptr.is_null() {
                break;
            }

            let v = unsafe { (*next).elem };

            if v >= value {
                if v == value {
                    matches = true;
                }
                break;
            }
            prev_ptr = next_ptr;       // prev ptr now points to next ptr          

            prev = rlu_dereference(self.rlu_global, self.thread_id, prev_ptr);

            next_ptr = unsafe { (*prev).next };

            next = rlu_dereference(self.rlu_global, self.thread_id, next_ptr);
        }

        if matches {
            break;
        }


        
        let tmp = rlu_global_obj.alloc(
          RluNode { 
            elem: value,
            next: ptr::null_mut()
          }
        );

        // create node
        let new_node_ptr = Box::into_raw(Box::new(tmp));


        let prev_ptr_locked = rlu_try_lock(self.rlu_global, self.thread_id, prev_ptr);






        if prev_ptr_locked.is_none() {
            rlu_abort(self.rlu_global, self.thread_id);
            continue;
        } 

        if !next_ptr.is_null() { // only if inserting to last 
          let next_ptr_locked = rlu_try_lock(self.rlu_global, self.thread_id, next_ptr);

          if next_ptr_locked.is_none() {
            rlu_abort(self.rlu_global, self.thread_id);
            continue;
          }
        }


        let plocked = prev_ptr_locked.unwrap();
        // let mut nlocked = next_ptr_locked.unwrap();

        unsafe {
            let new_locked = rlu_try_lock(self.rlu_global, self.thread_id, new_node_ptr).unwrap();

            (*new_locked).next = next_ptr;

            (*plocked).next = new_node_ptr;


            println!("prev, {:?}", (*plocked));
            println!("new , {:?}", (*new_locked));


        }

        break;
    }

    rlu_reader_unlock(self.rlu_global, self.thread_id);

    true
}


  fn delete(&self, value: T) -> bool {
    unimplemented!()
  }

  fn clone_ref(&self) -> Self {
    let thread_id = rlu_thread_init(self.rlu_global);
    RluSet { 
      head: self.head, 
      thread_id: thread_id, 
      rlu_global: self.rlu_global 
    }
  }
}
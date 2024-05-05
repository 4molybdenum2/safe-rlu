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

// pub struct RluInt64Wrapper {
//   pub obj : *mut Rlu<u64>,
//   pub rlu_global : *mut RluGlobal<u64>
// }


#[derive(Debug, Clone, Copy)]
pub struct RluNode<T>{
  elem: T,
  next: Option<Rlu<RluNode<T>>>,
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
          next: None,
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

    let head_ptr = &self.head as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

    let mut node_ptr = rlu_dereference(self.rlu_global, self.thread_id, head_ptr);

    let first_deref = true;

    loop {
            if node_ptr.is_null() {
              break;
            }
            if first_deref {
              let next_: Option<Rlu<RluNode<T>>> = unsafe{ (*node_ptr).next };

              let mut next_ptr = ptr::null_mut();

              if let Some(nex) = &next_{

                next_ptr = nex as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

              } else {
                break;
                
              }
              node_ptr = rlu_dereference(self.rlu_global, self.thread_id, next_ptr);

              continue;

            } else {
              let v = unsafe{ (*node_ptr).elem};

              if v > value {
                break;
              }

              if v == value {
                ret = true;
                break;
              }

              let next_: Option<Rlu<RluNode<T>>> = unsafe{ (*node_ptr).next };

              let next_ptr = &next_.unwrap() as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

              // increment ptr
              node_ptr = rlu_dereference(self.rlu_global, self.thread_id, next_ptr);
            }
    }

    rlu_reader_unlock(self.rlu_global, self.thread_id);
    ret
  }



  fn len(&self) -> usize {
      let mut len = 0;

      rlu_reader_lock(self.rlu_global, self.thread_id);

      let head_ptr = &self.head as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

      let mut node_ptr = rlu_dereference(self.rlu_global, self.thread_id, head_ptr);

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

          let next_: Option<Rlu<RluNode<T>>> = unsafe{ (*node_ptr).next };

          let mut next_ptr = ptr::null_mut();

          if let Some(nex) = &next_ {

            println!("{:?}", nex);

            next_ptr = nex as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;
            
          } else {

            break;
            
          }

          // increment ptr
          node_ptr = rlu_dereference(self.rlu_global, self.thread_id, next_ptr);
        }
      }

      rlu_reader_unlock(self.rlu_global, self.thread_id);
      
      len
    }

  fn insert(&mut self, value: T) -> bool {

     loop {
      rlu_reader_lock(self.rlu_global, self.thread_id);
        let mut prev_ptr = &self.head as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

        let mut prev = rlu_dereference(self.rlu_global, self.thread_id, prev_ptr);

        let next_deref: Option<Rlu<RluNode<T>>> = unsafe{ (*prev).next };


        let mut next_ptr = ptr::null_mut();

        if let Some(nex) = &next_deref {
          
            next_ptr = nex as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

        } else {

          // rlu_abort(self.rlu_global, self.thread_id);
          // continue;

        }
        //let mut next_ptr = &next_deref.unwrap() as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

        let mut next = rlu_dereference(self.rlu_global, self.thread_id, next_ptr);

        let mut matches = false;

        loop {
          if next.is_null() {
            break;
          }

          let v = unsafe { (*next).elem};


          if v >= value {
            if v == value {
              matches = true;
            }
            break;
          }

          prev_ptr = next_ptr;

          let next_to_next_deref: Option<Rlu<RluNode<T>>> = unsafe{ (*next).next };

          let next_to_next_ptr = &next_to_next_deref.unwrap() as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

          next = rlu_dereference(self.rlu_global, self.thread_id, next_to_next_ptr);
          
        }

        if matches {
          break;
        }


        let mut x = rlu_try_lock(self.rlu_global, self.thread_id, prev_ptr);

        if x.is_none() {
          rlu_abort(self.rlu_global, self.thread_id);
          continue;
        }

        if !next.is_null() {

          let mut x = rlu_try_lock(self.rlu_global, self.thread_id, next_ptr);

          if x.is_none() {
            rlu_abort(self.rlu_global, self.thread_id);
          }
        }

        // create new node and attach to list
        let new_node = Box::into_raw(
          Box::new(
            RluNode{
              elem: value,
              next: None,
            }
          )
        );




        let mut new_node_next_ptr = &(unsafe{ (*new_node).next}).unwrap() as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

        new_node_next_ptr = next_ptr;  


        let mut prev_next_ptr = &(unsafe{ (*prev).next}).unwrap() as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;

        prev_next_ptr = new_node_next_ptr;

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
use crate::concurrent_set::ConcurrentSet;
use crate::rlu::{
self, Rlu, RluGlobal, RluThreadData
};
use crate::{rlu_abort, rlu_dereference, rlu_reader_lock, rlu_reader_unlock, rlu_thread_init, rlu_try_lock};
use std::fmt::Debug;
use std::marker::{Unpin, PhantomData};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::{mem, ptr};


pub struct RluSet<T : 'static + Clone> {
  head: Option<Rlu<RluNode<T>>>,
  thread_id: usize,
  rlu_global: *mut RluGlobal<RluNode<T>>,
}

// pub struct RluInt64Wrapper {
//   pub obj : *mut Rlu<u64>,
//   pub rlu_global : *mut RluGlobal<u64>
// }


#[derive(Debug, Clone, Copy)]
struct RluNode<T>{
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
    let rlu_global_obj = unsafe {& *rlu_global};

    let thread_id = rlu_thread_init(rlu_global);

    RluSet{
      head:None,
      thread_id: thread_id,
      rlu_global: rlu_global,
    }
  }


  // pub fn rlu_new_node<T: Clone>(value: T) -> *mut RluNode<T> {
  //   let node = Box::new(RluNode {
        
  //       next: ptr::null_mut(),
  //       elem: value,
  //   });

  //   let tmp = Box::into_raw(node);
  //   tmp
  // }
  // pub fn get_thread_id(&self)->usize{
  //   let thread_id:usize = match &self.thread {
  //     Some(thread_data) => thread_data.thread_id(),
  //     None => panic!("Thread data not available"),
  //   };
  //   thread_id
  // }

  // pub fn to_string(&self) -> String {
  //   let mut result = String::new();
  //   let mut current: &Option<Box<RluNode<T>>> = &self.head;

  //   while let Some(node) = current{
  //     result.push_str(&format!("{:?}",node.elem));
  //     result.push_str("->");
  //     current = &node.next;
  //   }
  //   result
  // }
}


impl<T> ConcurrentSet<T> for RluSet<T> where T: PartialEq + PartialOrd + Copy + Clone + Debug + Unpin {
  
  fn contains(&self, value: T) -> bool {
    rlu_reader_lock(self.rlu_global, self.thread_id);

      // Dereference the head pointer
      if let Some(head) = self.head {
          let head_ptr = &head as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;
          let mut current_ptr = rlu_dereference(self.rlu_global, self.thread_id, head_ptr);

          // Traverse the linked list
          while let Some(current_node) = current_ptr {
              if unsafe {(*current_node).elem} == value {
                  rlu_reader_unlock(self.rlu_global, self.thread_id);
                  return true;
              }


              unsafe {
                let next_ptr = (*current_node).next.unwrap();
                current_ptr = rlu_dereference(self.rlu_global, self.thread_id, &next_ptr as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>);
              }
          }
      }

      rlu_reader_unlock(self.rlu_global, self.thread_id);
      false
  }



  fn len(&self) -> usize {
    
      let mut length: usize = 0;
      rlu_reader_lock(self.rlu_global, self.thread_id);


      if let Some(head) = self.head {
        let head_ptr = &head as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;
        let mut current_ptr = rlu_dereference(self.rlu_global, self.thread_id, head_ptr);

          while let Some(current_node) = current_ptr {
            length += 1;
            unsafe {
              let next_ptr = (*current_node).next.unwrap();
              current_ptr = rlu_dereference(self.rlu_global, self.thread_id, &next_ptr as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>);
            }
          }
      }

      rlu_reader_unlock(self.rlu_global, self.thread_id);

    length
    }

  fn insert(&mut self, value: T) -> bool {
     
    let rlu_global_obj = unsafe { & *self.rlu_global };
    'outer : loop{
      rlu_reader_lock(self.rlu_global, self.thread_id);

      if let Some(head) = self.head {
        let mut prev_ptr = &head as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;
        let mut prev = rlu_dereference(self.rlu_global, self.thread_id, prev_ptr);
        
        unsafe {
          let mut nex_ptr =  &(*prev.unwrap()).next.unwrap() as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;
          let mut nex =   rlu_dereference(self.rlu_global, self.thread_id, nex_ptr);
          //let next_ptr = rlu_dereference(self.rlu_global, self.thread_id, &nex as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>);

          'inner : loop {
            if nex.is_none() {
              break;
            }
            let next_ptr_deref = nex.unwrap();
            let v = (*next_ptr_deref).elem;

            if v == value {
              break 'outer; // present in list
            }

            if v >  value {
              break 'inner;
            }


            //update prev pointer
            prev_ptr = nex_ptr;

            // update next pointer
            let mut nex_to_nex_ptr: *mut Rlu<RluNode<T>> = &(*nex.unwrap()).next.unwrap() as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;
            //let nex_to_nex = rlu_dereference(self.rlu_global, self.thread_id, nex_to_nex_ptr);

            nex_ptr = nex_to_nex_ptr;

          }


          unsafe {
            let x: Option<*mut RluNode<T>> = rlu_try_lock(self.rlu_global, self.thread_id, prev_ptr);
            match x {
              None => {
                rlu_abort(self.rlu_global, self.thread_id);
                continue;
              }
              Some(x) => {
                // create new node and attach to set

                let rlu_new_node = rlu_global_obj.alloc(
                  RluNode {
                    elem: value,
                    next: None,
                  }
                );

                
                let mut new_ptr = &rlu_new_node as *const Rlu<RluNode<T>> as *mut Rlu<RluNode<T>>;;
                
                
                

                
              }
            }
          }
          
        }

      }
      //let mut prev = rlu_dereference(self.rlu_global, self.thread_id, self.head);

    }
    // let new_node = Box::new(RluNode{
    //   elem: value,
    //   next: self.head.take(),
    // });
    // self.head = Some(new_node);
    rlu_reader_unlock(self.rlu_global, self.thread_id);
    true
  }

  fn delete(&self, value: T) -> bool {
    let mut current:&Option<Box<RluNode<T>>>= &self.head;
    let mut prev:&Option<Box<RluNode<T>>>=current;

    while let Some(node)=current{
      if(node.elem)==value{
        return true;
      }
      current = &node.next;
    }
    return false;
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
use crate::concurrent_set::ConcurrentSet;
use crate::rlu::{
RluThreadData,RluGlobal,Rlu,
};
use crate::{rlu_dereference, rlu_reader_lock, rlu_reader_unlock};
use std::fmt::Debug;
use std::marker::{Unpin, PhantomData};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::mem;

pub struct RluSet<T> {
  head: Option<Box<RluNode<T>>>,
  thread: Option<RluThreadData<T>>,
  //rlu: Rlu<T>,
}

struct RluNode<T>{
  elem: T,
  next: Option<Box<RluNode<T>>>,
}


// In case you need raw pointers in your RluSet, you can assert that RluSet is definitely
// Send and Sync
unsafe impl<T> Send for RluSet<T> {}
unsafe impl<T> Sync for RluSet<T> {}

impl<T> RluSet<T> where T: PartialEq + PartialOrd + Copy + Clone + Debug + Unpin {
  pub fn new() -> RluSet<T> {
    RluSet{
      head: None,
      thread: None,
      //rlu,
    }
  }


  pub fn to_string(&self) -> String {
    let mut result = String::new();
    let mut current: &Option<Box<RluNode<T>>> = &self.head;

    while let Some(node) = current{
      result.push_str(&format!("{:?}",node.elem));
      result.push_str("->");
      current = &node.next;
    }
    result
  }
}


impl<T> ConcurrentSet<T> for RluSet<T> where T: PartialEq + PartialOrd + Copy + Clone + Debug + Unpin {
  fn contains(&self, value: T) -> bool {

    let g_rlu: *mut RluGlobal<T>=std::ptr::null_mut();
    let rlu_local: *mut Rlu<T>=std::ptr::null_mut();

    //Putting a RLU lock before reading the node
    let mut current:&Option<Box<RluNode<T>>> = &self.head;

    while let Some(node) = current{

      //Get the thread ID for the node
      let thread_id:usize = match &self.thread {
        Some(thread_data) => thread_data.thread_id(),
        None => panic!("Thread data not available"),
      };

      //Lock the node before reading it's value
      rlu_reader_lock(g_rlu,thread_id);

      //Use rlu_dereference to get it's value and compare
      //let rlu_ptr: *mut Rlu<T> = unsafe { mem::transmute(self.rlu) };
      let dereference_value = rlu_dereference(g_rlu, thread_id, rlu_local);

      unsafe {
        if let Some(deref_value_ptr) = dereference_value {
            if *deref_value_ptr == value {
                rlu_reader_unlock(g_rlu, thread_id);
                return true;
            }
        }
    }

      //Unlock the node
        rlu_reader_unlock(g_rlu,thread_id);
      current = &node.next;
    }
    false
  }

  fn len(&self) -> usize {
    
    let mut length: usize = 0;
    let mut current:&Option<Box<RluNode<T>>> = &self.head;

    while let Some(node) = current{
      length +=1;
      current = &node.next;
  }
  length

  }

  fn insert(&mut self, value: T) -> bool {
    let new_node = Box::new(RluNode{
      elem: value,
      next: self.head.take(),
    });
    self.head = Some(new_node);
    
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
    unimplemented!()
  }
}
use crate::concurrent_set::ConcurrentSet;
use std::fmt::Debug;
use std::marker::{Unpin, PhantomData};


pub struct RluSet<T> {
  head: Link<T>,
}

type Link<T> = Option<Box<Node<T>>>;

struct Node<T>{
  elem: T,
  next: Link<T>,
}
// In case you need raw pointers in your RluSet, you can assert that RluSet is definitely
// Send and Sync
unsafe impl<T> Send for RluSet<T> {}
unsafe impl<T> Sync for RluSet<T> {}

impl<T> RluSet<T> where T: PartialEq + PartialOrd + Copy + Clone + Debug + Unpin {
  pub fn new() -> RluSet<T> {
    RluSet{
      head: None,
    }
  }


  pub fn to_string(&self) -> String {
    let mut result = String::new();
    let mut current = &self.head;

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
    let mut current = &self.head;
    while let Some(node) = current{
      if node.elem==value{
        return true;
      }
      current = &node.next;
    }
    false
  }

  fn len(&self) -> usize {
    
    let mut length = 0;
    let mut current = &self.head;

    while let Some(node) = current{
      length +=1;
      current = &node.next;
  }
  length

  }

  fn insert(&self, value: T) -> bool {
    //Why tf are we returning a boolean?
    let new_node = Box::new(Node{
      elem: value,
      next: self.head.take(),
    });
    self.head = Some(new_node);
    true
  }

  fn delete(&self, value: T) -> bool {
    let mut current = &mut self.head;
    let mut prev: &mut Link<T> = &mut None;

    while let Some(node) = current{
      if node.elem == value{
        if let Some(prev_node) = prev{
          unimplemented!();
        }else{
          self.head = node.next.take();
        }
        return true;
      }
    current = &mut node.next;
    prev = Some(&mut node.next);
    }
    false
  }

  fn clone_ref(&self) -> Self {
    unimplemented!()
  }
}
use crate::concurrent_set::ConcurrentSet;
use std::fmt::Debug;
use std::marker::{Unpin, PhantomData};


pub struct RluSet<T> {
  head: Option<Box<Node<T>>>,
}

struct Node<T>{
  elem: T,
  next: Option<Box<Node<T>>>,
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
    let mut current: &Option<Box<Node<T>>> = &self.head;

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
    let mut current:&Option<Box<Node<T>>> = &self.head;

    while let Some(node) = current{
      if node.elem==value{
        return true;
      }
      current = &node.next;
    }
    false
  }

  fn len(&self) -> usize {
    
    let mut length: usize = 0;
    let mut current:&Option<Box<Node<T>>> = &self.head;

    while let Some(node) = current{
      length +=1;
      current = &node.next;
  }
  length

  }

  fn insert(&mut self, value: T) -> bool {
    let new_node = Box::new(Node{
      elem: value,
      next: self.head.take(),
    });
    self.head = Some(new_node);
    true
  }

  fn delete(&self, value: T) -> bool {
    let mut current:&Option<Box<Node<T>>>= &self.head;
    let mut prev:&Option<Box<Node<T>>>=current;

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
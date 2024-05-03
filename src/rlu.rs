#![allow(dead_code, unused_variables)]

use std::fmt::Debug;
use std::mem::MaybeUninit;
use std::ptr;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::usize;

// Constants
const RLU_MAX_LOG_SIZE: usize = 128;
const RLU_MAX_THREADS: usize = 32;
const RLU_MAX_FREE_NODES: usize = 100;



macro_rules! debug_log {
    ($($rest:tt)*) => {
        #[cfg(debug_assertions)]
        std::println!($($rest)*);
    }
}


#[derive(Debug)]
pub struct ObjOriginal<T> {
    copy : AtomicPtr<ObjCopy<T>>,
    data : T,
}


#[derive(Debug)]
pub struct ObjCopy<T> {
    thread_id : usize,
    original : Rlu<T>,
    data : T,
}


unsafe impl<T> Send for Rlu<T> {}
unsafe impl<T> Sync for Rlu<T> {}


#[derive(Debug)]
pub struct Rlu<T> (
    *mut ObjOriginal<T>
);

pub trait ClonedT : Clone {} 
impl<T: Clone> ClonedT for T {}

impl<T> Clone for Rlu<T> {
    fn clone(&self) -> Self{
        *self
    }
}

impl<T> Copy for Rlu<T> {}

impl<T> Rlu<T> {
    pub fn deref(&self) -> &ObjOriginal<T>{
        unsafe {
            &*self.0
        }
    }

    pub fn deref_mut(&self) -> &mut ObjOriginal<T> {
        unsafe {
            &mut *self.0
        }
    }
}
pub struct WriteLog<T> {
    log : [ObjCopy<T>; RLU_MAX_LOG_SIZE],
    curr_size : usize,
}


impl<T> WriteLog<T> {
    fn new() -> WriteLog<T> {
        WriteLog {
            log: unsafe{ MaybeUninit::uninit().assume_init() },
            curr_size: 0,
        }
    }
}

pub struct RluThreadData<T> {
    is_writer : bool,
    write_clock : AtomicUsize,
    local_clock : AtomicUsize,
    run_cnt : AtomicUsize,
    write_log : [WriteLog<T>; 2],
    current_log: usize,
    thread_id : usize,
    free_nodes : [Rlu<T>; RLU_MAX_FREE_NODES],
    free_nodes_size : usize,
}

impl<T> RluThreadData<T> {
    fn new(thid : usize) -> RluThreadData<T> {
        RluThreadData {
            is_writer: false,
            write_clock: AtomicUsize::new(usize::MAX),
            local_clock: AtomicUsize::new(0),
            run_cnt: AtomicUsize::new(0),
            write_log: [WriteLog::new(), WriteLog::new()], // create a current log and a swap log
            current_log: 0,
            thread_id: thid,
            free_nodes: unsafe{MaybeUninit::uninit().assume_init()},
            free_nodes_size: 0,
        }
    }
}

pub struct RluGlobal<T : ClonedT> {
    global_clock : AtomicUsize,
    n_threads : AtomicUsize,
    threads : [RluThreadData<T> ; RLU_MAX_THREADS],
}


impl<T : ClonedT> RluGlobal<T> {
    fn new() -> RluGlobal<T> {
        
        RluGlobal {
            global_clock: AtomicUsize::new(0),
            n_threads: AtomicUsize::new(0),
            threads: unsafe {MaybeUninit::uninit().assume_init()},
        }
    }

    pub fn init() -> *mut RluGlobal<T> {
        let boxed = Box::new(RluGlobal::new());
        Box::into_raw(boxed)
    }

    pub fn alloc(&self, data : T) -> Rlu<T> {
        Rlu(
            Box::into_raw(
                Box::new(ObjOriginal {
                    copy: AtomicPtr::new(ptr::null_mut()),
                    data,
                    }
                )
            )
        )
    }

    
        
    }

pub fn rlu_thread_init<T : ClonedT> (rlu_global: *mut RluGlobal<T>) -> usize {
    unsafe {
        let thread_id = (*rlu_global).n_threads.fetch_add(1, Ordering::SeqCst);
        assert!(thread_id < RLU_MAX_THREADS);
        
        let thread_data = RluThreadData::new(thread_id);
        (*rlu_global).threads[thread_id] = thread_data;
        
        thread_id
    }
}



pub fn rlu_reader_lock<T : ClonedT>(g_rlu: *mut RluGlobal<T>, thread_id: usize) {
    debug_log!("Thread {thread_id}: lock");
    unsafe {
        if !g_rlu.is_null() { // Safety check
            if thread_id < RLU_MAX_THREADS {
                let rlu_global = &mut *g_rlu;
                let thread_data = &mut rlu_global.threads[thread_id];
                
                assert_eq!(thread_data.run_cnt.load(Ordering::SeqCst) & 0x1, 0);

                thread_data.is_writer = false;
                thread_data.run_cnt.fetch_add(1, Ordering::SeqCst);
                thread_data.local_clock.store(rlu_global.global_clock.load(Ordering::SeqCst), Ordering::SeqCst);
            }
        } else {
            panic!("Thread ID out of bounds...");
        }
    }
}


pub fn rlu_reader_unlock<T : ClonedT>(g_rlu: *mut RluGlobal<T>, thread_id: usize) {
    debug_log!("Thread {thread_id}: unlock");
    unsafe {
        if !g_rlu.is_null() { // Safety check
            if thread_id < RLU_MAX_THREADS {
                let rlu_global = &mut *g_rlu;
                let thread_data = &mut rlu_global.threads[thread_id];

                assert_ne!(thread_data.run_cnt.load(Ordering::SeqCst) & 0x1, 0);
                thread_data.run_cnt.fetch_add(1, Ordering::SeqCst);
                if thread_data.is_writer {
                    thread_data.is_writer = false;
                    rlu_commit_write_log(g_rlu, thread_id);
                }
            } else {
                panic!("Thread ID out of bounds...");
            }
        }
    }
}

pub fn rlu_dereference<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize, obj : *mut Rlu<T>) -> Option<*mut T> {
    debug_log!("Thread {thread_id}: dereference");
    unsafe {
        let actual_obj: &mut ObjOriginal<T> = (*obj).deref_mut();
        let copy = actual_obj.copy.load(Ordering::SeqCst).as_mut();
        match copy {
            None => { 
                debug_log!("return original");
                Some(&mut actual_obj.data as *mut T)
            }

            Some(copy) => {
                let lockthd = copy.thread_id;
                if thread_id == lockthd {
                    debug_log!("deref self?");
                    return Some(&mut copy.data as *mut T);
                } else {
                    
                    let other_write_clock = (*g_rlu).threads[lockthd].write_clock.load(Ordering::SeqCst); // get other write lock


                    let my_local_clock = (*g_rlu).threads[thread_id].write_clock.load(Ordering::SeqCst);// get our own local clock


                    if other_write_clock <= my_local_clock {
                        debug_log!("deref other copy?");
                        return Some(&mut copy.data as *mut T);
                    }

                    debug_log!("deref original");
                    return Some(&mut actual_obj.data as *mut T);
                }
            }
        }
    }
}

pub fn rlu_try_lock<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize, obj: *mut Rlu<T>) -> Option<*mut T>{
    debug_log!("Thread {thread_id}: try lock for write");
    unsafe {
        if !g_rlu.is_null() { // Safety check
            if thread_id < RLU_MAX_THREADS {
                let rlu_global = &mut *g_rlu;
                //let thread_data = &mut rlu_global.threads[thread_id];

                rlu_global.threads[thread_id].is_writer = true;
                let actual_obj = (*obj).deref();
                // get copy from original;
                if let Some(ptr_copy) = (*obj).deref_mut().copy.load(Ordering::SeqCst).as_mut() {
                    // locked
                    let thr_id = ptr_copy.thread_id;
                    
                    if thread_id == thr_id {

                        if rlu_global.threads[thr_id].run_cnt.load(Ordering::SeqCst)  == rlu_global.threads[thread_id].run_cnt.load(Ordering::SeqCst) {
                            debug_log!("Tried locking from same execution of thread");
                            return Some(&mut ptr_copy.data as *mut T);
                        }
                        
                        return None;
                    } 
                    return None;
                } 
                
                // Append to ptr_copy log
                let active_log = &mut rlu_global.threads[thread_id].write_log[rlu_global.threads[thread_id].current_log];
                let curr_idx = active_log.curr_size;
                //let next_idx = active_log.curr_size + 1;
                active_log.curr_size += 1;
                let copy_obj = active_log.log.get_unchecked_mut(curr_idx);
                copy_obj.thread_id = thread_id;
                copy_obj.original = *obj;
                copy_obj.data = actual_obj.data.clone();
                


                let prev = actual_obj.copy.compare_and_swap(ptr::null_mut(), copy_obj, Ordering::SeqCst);
                if prev != ptr::null_mut() {
                    // failed
                    active_log.curr_size -= 1;
                    return None;
                }

                return Some(&mut copy_obj.data as *mut T);

            } else {
                panic!("Thread ID out of bounds...");
            }
        }
        panic!("Global RLU is null...");
    }
}


pub fn rlu_commit_write_log<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize) {
    debug_log!("Thread {thread_id}: commit write log");
    unsafe {
        if !g_rlu.is_null() { // safety check

            if thread_id < RLU_MAX_THREADS {
                let rlu_global = &mut *g_rlu;
                let thread_data = &mut rlu_global.threads[thread_id];


                thread_data.write_clock.store(rlu_global.global_clock.load(Ordering::SeqCst) + 1, Ordering::SeqCst);
                rlu_global.global_clock.fetch_add(1, Ordering::SeqCst);
               

            } else {
                    panic!("Thread ID out of bounds...");
            }
        }   
    }


    // synchronize
    rlu_synchronize(g_rlu, thread_id); // must drain readers


    // writeback and unlock
    rlu_writeback_write_log(g_rlu, thread_id);

    rlu_unlock_write_log(g_rlu, thread_id);

    unsafe {
        if !g_rlu.is_null() { // safety check

            if thread_id < RLU_MAX_THREADS {

                let rlu_global = &mut *g_rlu;
                let thread_data = &mut rlu_global.threads[thread_id];
                
                thread_data.write_clock.store(std::u64::MAX as usize, Ordering::SeqCst);
               

            } else {
                    panic!("Thread ID out of bounds...");
            }
        } 
    }
    //swap write logs
    rlu_swap_write_logs(g_rlu, thread_id);

    // process free
    rlu_process_free(g_rlu, thread_id);

}


pub fn rlu_synchronize<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize){
    debug_log!("Thread {thread_id}: sync");
    unsafe {
        let rlu_global = &mut *g_rlu;
        let thread = &rlu_global.threads[thread_id];

        let n = rlu_global.n_threads.load(Ordering::SeqCst);
        let sync_cnts: Vec<usize> = (0..n)
            .map(|i| rlu_global.threads[i].run_cnt.load(Ordering::SeqCst))
            .collect();

        for i in 0..n {
            if i == thread_id {
                continue;
            }

            let other: &RluThreadData<T> = &rlu_global.threads[i];
            loop {
                if sync_cnts[i] % 2 == 0 {
                    debug_log!("Thread {thread_id} d0");
                    break;
                }

                if other.run_cnt.load(Ordering::SeqCst) != sync_cnts[i] {
                    debug_log!("Thread {thread_id} d1");
                    break;
                }

                if thread.write_clock.load(Ordering::SeqCst) <= other.local_clock.load(Ordering::SeqCst) {
                    debug_log!("Thread {thread_id} d2");
                    break;
                }
            }
            
        }

    }
}


pub fn rlu_swap_write_logs<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize) {
    debug_log!("Thread {thread_id}: swap write log");
    unsafe {
        let rlu_global = &mut *g_rlu;
        let thread_data = &mut rlu_global.threads[thread_id];

        thread_data.current_log = (thread_data.current_log + 1)%2;
        let curr_log = &mut thread_data.write_log[thread_data.current_log];
        curr_log.curr_size = 0; // start from log beginning, which basically means empty log
        
        
    }
}

pub fn rlu_abort<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize) {
    debug_log!("Thread {thread_id}: abort");
    unsafe {
        if !g_rlu.is_null() { // safety check
            // abort when lock failed and we will retry from same thread
            // basically makes run_cnt even again
            if thread_id < RLU_MAX_THREADS {
                let rlu_global = &mut *g_rlu;
                let thread_data = &mut rlu_global.threads[thread_id];
                let cnt = thread_data.run_cnt.fetch_add(1, Ordering::SeqCst);
                assert_ne!((cnt & 0x1), 0);
                if thread_data.is_writer {
                    // unlock write log here
                    thread_data.is_writer = false;
                    rlu_unlock_write_log(g_rlu, thread_id);
                }
                

                // retry code - can be done from specific data str library(?)
            } else {
                    panic!("Thread ID out of bounds...");
            }
        }   
    }
}


pub fn rlu_writeback_write_log<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize) {
    debug_log!("Thread {thread_id}: writeback write log");
    unsafe {
        let rlu_global = &mut *g_rlu;
        let thread_data = &mut rlu_global.threads[thread_id];
            
        let curr_log =&mut thread_data.write_log[thread_data.current_log];
        for i in 0..curr_log.curr_size {
            let copy = &mut curr_log.log[i];
            
            let actual = copy.original.deref_mut();
            actual.data = copy.data.clone();
        }
        

    }

}


pub fn rlu_unlock_write_log<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize) {
    debug_log!("Thread {thread_id}: unlock write log");
    unsafe {
        let rlu_global = &mut *g_rlu;
        let thread_data = &mut rlu_global.threads[thread_id];
            
        let curr_log =&mut thread_data.write_log[thread_data.current_log];
        for i in 0..curr_log.curr_size {
            let copy = &mut curr_log.log[i];
            let actual = copy.original.deref_mut();
            actual.copy.store(null_mut(), Ordering::SeqCst);
        
        }

        curr_log.curr_size = 0;
    }
}

/* this is just for dropping the objects added to free */
pub fn rlu_process_free<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize) {
    debug_log!("Thread {thread_id}: process free");
    unsafe {
        let rlu_global = &mut *g_rlu;
        let thread_data = &mut rlu_global.threads[thread_id];

        for i in 0..thread_data.free_nodes_size {
            Box::from_raw(thread_data.free_nodes[i].0); //deallocate memory - hack
        }
        thread_data.free_nodes_size = 0;
    }
}



/* this is for freeing objects*/
pub fn rlu_free<T : ClonedT>(g_rlu : * mut RluGlobal<T>, thread_id : usize, obj : *mut Rlu<T>) {
    debug_log!("Thread {thread_id}: free");

    unsafe {
        let rlu_global = &mut *g_rlu;
        let thread_data = &mut rlu_global.threads[thread_id];
        

        let fid = thread_data.free_nodes_size;
        thread_data.free_nodes_size += 1;
        thread_data.free_nodes[fid] = *obj; // not sure if this will work, otherwise we have to pass the mutable pointer
    }

}
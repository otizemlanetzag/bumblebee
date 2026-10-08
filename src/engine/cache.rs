use std::{collections::HashMap,time::{Duration,Instant}};use parking_lot::RwLock;
pub struct HttpCache{entries:RwLock<HashMap<String,(Instant,Vec<u8>)>>,ttl:Duration}
impl HttpCache{pub fn new(ttl:Duration)->Self{Self{entries:RwLock::new(HashMap::new()),ttl}}pub fn get(&self,k:&str)->Option<Vec<u8>>{let r=self.entries.read();let (t,b)=r.get(k)?;if t.elapsed()<self.ttl{Some(b.clone())}else{None}}pub fn put(&self,k:String,b:Vec<u8>){self.entries.write().insert(k,(Instant::now(),b));}pub fn clear(&self){self.entries.write().clear();}}

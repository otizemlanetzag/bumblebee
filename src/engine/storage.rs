use std::collections::HashMap;use parking_lot::RwLock;
#[derive(Default)]pub struct Storage{values:RwLock<HashMap<(String,String),String>>}
impl Storage{pub fn set(&self,o:&str,k:&str,v:String){self.values.write().insert((o.into(),k.into()),v);}pub fn get(&self,o:&str,k:&str)->Option<String>{self.values.read().get(&(o.into(),k.into())).cloned()}pub fn remove(&self,o:&str,k:&str){self.values.write().remove(&(o.into(),k.into()));}pub fn clear_origin(&self,o:&str){self.values.write().retain(|(x,_),_|x!=o);}}

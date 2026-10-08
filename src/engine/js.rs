#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum ScriptStatus{Disabled,Pending,Executed,Failed}
pub trait JavaScriptRuntime:Send+Sync{fn evaluate(&self,source:&str)->Result<(),String>;}
pub struct NoopJavaScript;impl JavaScriptRuntime for NoopJavaScript{fn evaluate(&self,_:&str)->Result<(),String>{Ok(())}}

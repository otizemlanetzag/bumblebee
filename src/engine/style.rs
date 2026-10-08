use std::collections::HashMap;
#[derive(Clone,Debug)] pub struct Style{pub display:Display,pub position:Position,pub width:Length,pub height:Length,pub margin:Edges,pub padding:Edges,pub color:Color,pub background:Color,pub font_size:f32}
#[derive(Clone,Copy,Debug,Default,PartialEq)] pub enum Display{#[default] Block,Inline,None}
#[derive(Clone,Copy,Debug,Default,PartialEq)] pub enum Position{#[default] Static,Relative,Absolute,Fixed}
#[derive(Clone,Copy,Debug,Default,PartialEq)] pub enum Length{#[default] Auto,Px(f32),Percent(f32)}
#[derive(Clone,Copy,Debug,Default)] pub struct Edges{pub top:f32,pub right:f32,pub bottom:f32,pub left:f32}
#[derive(Clone,Copy,Debug,Default)] pub struct Color{pub r:u8,pub g:u8,pub b:u8,pub a:u8}
impl Color{pub const WHITE:Self=Self{r:255,g:255,b:255,a:255};pub const BLACK:Self=Self{r:0,g:0,b:0,a:255};}
pub type StyleMap=HashMap<usize,Style>;
pub fn default_style()->Style{Style{display:Display::Block,position:Position::Static,width:Length::Auto,height:Length::Auto,margin:Edges::default(),padding:Edges::default(),color:Color::BLACK,background:Color::WHITE,font_size:16.}}

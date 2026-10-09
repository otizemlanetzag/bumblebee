use super::css::{ComputedStyle, CssLength};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect { pub x:f32, pub y:f32, pub width:f32, pub height:f32 }
#[derive(Clone, Debug)]
pub struct LayoutBox { pub node_id:usize, pub rect:Rect, pub style:ComputedStyle, pub text:Option<String>, pub children:Vec<LayoutBox> }

fn len(v:&CssLength,base:f32,font:f32,viewport:(f32,f32))->f32 {
    match v {
        CssLength::Px(x)=>*x, CssLength::Percent(x)=>base*(*x)/100.0,
        CssLength::Em(x)=>font*(*x), CssLength::Rem(x)=>16.0*(*x),
        CssLength::Vw(x)=>viewport.0*(*x)/100.0, CssLength::Vh(x)=>viewport.1*(*x)/100.0,
        CssLength::Vmin(x)=>viewport.0.min(viewport.1)*(*x)/100.0,
        CssLength::Vmax(x)=>viewport.0.max(viewport.1)*(*x)/100.0,
        CssLength::Zero=>0.0, CssLength::Auto|CssLength::Calc(_)=>base,
    }
}
fn px(v:&CssLength,base:f32,font:f32,viewport:(f32,f32))->f32 { len(v,base,font,viewport).max(0.0) }

pub fn layout_node(id:usize,style:ComputedStyle,mut children:Vec<LayoutBox>,origin:(f32,f32),available:(f32,f32),viewport:(f32,f32))->LayoutBox {
    let width=if style.width==CssLength::Auto {available.0} else {len(&style.width,available.0,style.font_size,viewport).max(0.0)};
    let explicit_height=style.height!=CssLength::Auto;
    let height=if explicit_height {len(&style.height,available.1,style.font_size,viewport).max(0.0)} else {0.0};
    let gap=px(&style.grid_gap,width,style.font_size,viewport);

    if style.display=="flex" || style.display=="inline-flex" {
        let row=!style.flex_direction.starts_with("column");
        let available_main=if row {width}else{available.1};
        let mut used=0.0; let mut total_grow=0.0; let mut total_shrink=0.0;
        for c in &children {
            let base=if row {c.rect.width}else{c.rect.height};
            used+=base;
            total_grow+=c.style.flex_grow.max(0.0);
            total_shrink+=c.style.flex_shrink.max(0.0)*base;
        }
        if children.len()>1 {used+=gap*(children.len()-1) as f32;}
        let free=available_main-used;
        if free>0.0 && total_grow>0.0 {
            for c in &mut children {
                let add=free*c.style.flex_grow.max(0.0)/total_grow;
                if row {c.rect.width+=add;} else {c.rect.height+=add;}
            }
        } else if free<0.0 && total_shrink>0.0 {
            for c in &mut children {
                let base=if row {c.rect.width}else{c.rect.height};
                let reduce=(-free)*c.style.flex_shrink.max(0.0)*base/total_shrink;
                if row {c.rect.width=(base-reduce).max(0.0);} else {c.rect.height=(base-reduce).max(0.0);}
            }
        }
        let final_used: f32=children.iter().map(|c|if row{c.rect.width}else{c.rect.height}).sum::<f32>()+gap*(children.len().saturating_sub(1) as f32);
        let leftover=(available_main-final_used).max(0.0);
        let (start,extra_gap)=match style.justify_content.as_str() {
            "center"=>(leftover/2.0,gap),
            "flex-end"|"end"=>(leftover,gap),
            "space-between" if children.len()>1=>(0.0,gap+leftover/(children.len()-1) as f32),
            "space-around" if !children.is_empty()=>(leftover/(children.len() as f32*2.0),gap+leftover/children.len() as f32),
            "space-evenly" if !children.is_empty()=>(leftover/(children.len() as f32+1.0),gap+leftover/(children.len() as f32+1.0)),
            _=>(0.0,gap),
        };
        let mut cursor=start; let mut cross_max=0.0f32;
        for c in &mut children {
            let cross=if row {c.rect.height}else{c.rect.width}; cross_max=cross_max.max(cross);
            let cross_available=if row {height.max(cross)}else{width};
            let cross_pos=match style.align_items.as_str() {
                "center"=>((cross_available-cross)/2.0).max(0.0),
                "flex-end"|"end"=> (cross_available-cross).max(0.0),
                _=>0.0,
            };
            if row {c.rect.x=origin.0+cursor;c.rect.y=origin.1+cross_pos;}
            else {c.rect.x=origin.0+cross_pos;c.rect.y=origin.1+cursor;}
            cursor+=if row {c.rect.width}else{c.rect.height}; cursor+=extra_gap;
        }
        let content_height=if explicit_height {height} else if row {cross_max.max(1.0)} else {final_used.max(1.0)};
        return LayoutBox{node_id:id,rect:Rect{x:origin.0,y:origin.1,width,height:content_height},style,text:None,children};
    }

    if style.display=="grid" || style.display=="inline-grid" {
        let columns=style.grid_template_columns.split_whitespace().filter(|s|*s!="none").count().max(1);
        let col_width=((width-gap*(columns.saturating_sub(1) as f32))/columns as f32).max(0.0);
        let mut row_heights=vec![0.0f32; (children.len()+columns-1)/columns];
        for (i,c) in children.iter().enumerate(){row_heights[i/columns]=row_heights[i/columns].max(c.rect.height);}
        let mut row_y=0.0;
        for (i,c) in children.iter_mut().enumerate(){
            let row=i/columns; let col=i%columns;
            c.rect.x=origin.0+col as f32*(col_width+gap);
            c.rect.y=origin.1+row_y;
            c.rect.width=col_width;
            if row<row_heights.len() {c.rect.height=c.rect.height.max(row_heights[row]);}
            if col==columns-1 || i+1==children.len(){row_y+=row_heights[row]+gap;}
        }
        let content_height=if explicit_height{height}else{row_y.max(1.0)};
        return LayoutBox{node_id:id,rect:Rect{x:origin.0,y:origin.1,width,height:content_height},style,text:None,children};
    }

    let margin=px(&style.margin,width,style.font_size,viewport);
    let padding=px(&style.padding,width,style.font_size,viewport);
    let mut y=origin.1+margin+padding;
    for c in &mut children {
        c.rect.x=origin.0+margin+padding;
        c.rect.y=y;
        y+=c.rect.height+margin;
        if c.rect.width>width {c.rect.width=width;}
    }
    let content_height=if explicit_height{height}else{(y-origin.1+padding).max(1.0)};
    LayoutBox{node_id:id,rect:Rect{x:origin.0,y:origin.1,width,height:content_height},style,children}
}

pub fn layout_root(id:usize,style:ComputedStyle,viewport:(f32,f32))->LayoutBox {
    layout_node(id,style,Vec::new(),(0.0,0.0),viewport,viewport)
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn flex_grow_distributes_remaining_space() {
   let mut a=LayoutBox{node_id:1,rect:Rect{x:0.0,y:0.0,width:100.0,height:20.0},style:ComputedStyle{flex_grow:1.0,..ComputedStyle::default()},text:None,children:vec![]};
   let mut b=a.clone(); b.node_id=2;
   let parent=ComputedStyle{display:"flex".into(),..ComputedStyle::default()};
   let out=layout_node(0,parent,vec![a.clone(),b.clone()],(0.0,0.0),(400.0,100.0),(400.0,100.0));
   assert!((out.children[0].rect.width-200.0).abs()<0.01);
   assert!((out.children[1].rect.width-200.0).abs()<0.01);
 }
}

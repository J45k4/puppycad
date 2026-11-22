import "cad/project-assembly.flex";

global puppy_connector_slot=-1;

fn puppy_connector_row(row) {
    return row && m_eq(load64(row+16),"connector") && m_eq(j_string(puppy_design_owner(row),"type"),"assembly");
}
fn puppy_connector_group(slot) {if slot<3 {return "position";}return "rotation";}
fn puppy_connector_caption(owner,slot) {
    let space="World ";if m_len(owner) {space="Local ";}
    let field="position (mm)";if slot>=3 {field="rotation (°)";}
    return m_cat(space,field);
}
fn puppy_connector_cell(frame,slot) {
    return pc_pose_number(j_get(j_get(frame,puppy_connector_group(slot)),puppy_placement_axis(slot)));
}
fn puppy_connector_field(project,row,slot) {
    if !puppy_connector_row(row) || puppy_design_owner(row)!=cad_project_item(project) {return 0;}
    let node=load64(row+24);
    let owner=j_string(node,"instanceId");
    let frame=cad_project_connector_frame(project,owner,j_string(node,"id"));
    if !frame {return 0;}
    let label=m_cat(load64(row),m_cat(" / ",m_cat(puppy_connector_caption(owner,slot),m_cat(" / ",puppy_placement_axis(slot)))));
    return cg_v(node,puppy_connector_cell(frame,slot),label);
}
fn puppy_connector_begin(project,row,slot) {
    let field=puppy_connector_field(project,row,slot);
    if !field {return puppy_title(project,cg_error);}
    puppy_connector_slot=slot;
    return puppy_number_open(project,field);
}
fn puppy_connector_column(w) {return (w-20)/3;}
fn puppy_connector_row_y(h,row) {return h-62+row*37;}
fn puppy_connector_grid(rgb,w,h,project,row) {
    let node=load64(row+24);let owner=j_string(node,"instanceId");
    let frame=cad_project_connector_frame(project,owner,j_string(node,"id"));
    if !frame {return puppy_ui_text(rgb,w,h,13,h-51,cg_error,w-8);}
    let width=puppy_connector_column(w);let i=0;
    while i<6 {
        let x=10+i%3*width;let y=puppy_connector_row_y(h,i/3);
        if i%3==0 {
            puppy_ui_text(rgb,w,h,13,y-3,puppy_connector_caption(owner,i),w-8);
        }
        let label=m_cat(puppy_placement_axis(i),m_cat(" ",puppy_display_number(puppy_connector_cell(frame,i))));
        puppy_ui_rect(rgb,w,h,x,y,width-3,20,48,68,70);
        puppy_ui_text(rgb,w,h,x+5,y+15,label,x+width-4);
        i=i+1;
    }
    return 1;
}
fn puppy_connector_click(project,row,x,y,w,h) {
    let width=puppy_connector_column(w);let col=(x-10)/width;
    if x<10 || col>=3 || (x-10)%width>=width-3 {return 1;}
    let first=puppy_connector_row_y(h,0);let second=puppy_connector_row_y(h,1);
    if y>=first && y<first+20 {return puppy_connector_begin(project,row,col);}
    if y>=second && y<second+20 {return puppy_connector_begin(project,row,col+3);}
    return 1;
}

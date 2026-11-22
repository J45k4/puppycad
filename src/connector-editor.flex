import "cad/project-assembly.flex";

global puppy_connector_slot=-1;
global puppy_connector_rename=0;

fn puppy_connector_row(row) {
    return row && m_eq(load64(row+16),"connector") && m_eq(j_string(puppy_design_owner(row),"type"),"assembly");
}
fn puppy_connector_key(owner,id) {
    return m_cat(puppy_design_key("owner",owner),puppy_design_key("connector",id));
}
fn puppy_connector_reassign(project,node,owner,id) {
    let updated=cad_project_reassign_connector(project,j_string(node,"instanceId"),j_string(node,"id"),owner,id);
    if !updated || !puppy_refresh(updated) {return 0;}
    let assembly=puppy_design_key("assembly",j_string(cad_project_item(updated),"id"));
    let group=puppy_design_key("group","Connectors");
    let key=m_cat(assembly,m_cat("/",m_cat(group,m_cat("/",puppy_connector_key(owner,id)))));
    puppy_design_select(key);puppy_push(updated);return updated;
}
fn puppy_connector_rename_begin(project,node) {
    puppy_connector_rename=1;return puppy_text_open(project,j_string(node,"id"));
}
fn puppy_connector_rename_apply(project,text) {
    let row=puppy_design_selected(puppy_design_rows(project));
    if !puppy_connector_row(row) {return puppy_title(project,"Select a connector");}
    let node=load64(row+24);let end=m_len(text);
    while end && load8(text+end-1)<=32 {end=end-1;}
    while end && load8(text)<=32 {text=text+1;end=end-1;}
    let id=m_slice(text,end);
    if m_eq(j_string(node,"id"),id) {puppy_number_close();return puppy_title(project,"Value unchanged");}
    let updated=puppy_connector_reassign(project,node,j_string(node,"instanceId"),id);
    if !updated {return puppy_title(project,cg_error);}
    puppy_number_close();return puppy_title(updated,m_cat("Connector ID changed to ",id));
}
fn puppy_connector_attach_begin(project,node) {
    return puppy_assembly_choice_open(project,2,node,m_cat("Attach ",m_cat(puppy_design_name(node,"connector")," to an occurrence or World")));
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

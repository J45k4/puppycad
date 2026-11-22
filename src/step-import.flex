// Source bytes and the approved preview survive event arena resets.
import "cad/step-assembly.flex";
global puppy_step_input=0;
global puppy_step_catalog=0;
global puppy_step_filename=0;
global puppy_step_digest=0;
global puppy_step_preview=0;
global puppy_step_index=0;
global puppy_step_replace=-1;
global puppy_step_path_entry=0;
global puppy_step_view=0;
global puppy_step_has_tree=0;
global puppy_step_whole=0;

fn puppy_step_basename(path) {
    let start=path;let p=path;while load8(p) {if load8(p)==47 {start=p+1;}p=p+1;}return start;
}
fn puppy_step_extension(path) {
    let base=puppy_step_basename(path);let i=m_len(base);let dot=i;
    while dot && load8(base+dot)!=46 {dot=dot-1;}
    let ext=m_slice(base+dot,i-dot);let k=0;while load8(ext+k) {let c=load8(ext+k);if c>=65 && c<=90 {store8(ext+k,c+32);}k=k+1;}
    if m_eq(ext,".step") || m_eq(ext,".stp") {return base+dot-path;}return -1;
}
fn puppy_step_sha(bytes) {
    let lib=ffi_open("libcrypto.so.3");if !lib {return cg_fail("STEP import needs libcrypto.so.3");}
    let sha=ffi_symbol(lib,"SHA256");let hash=m_take(32);let ok=sha && ffi_call(sha,bytes,m_len(bytes),hash,0,0,0);ffi_close(lib);
    if !ok {return cg_fail("Cannot hash STEP source");}
    let text=m_take(65);let i=0;let hex="0123456789abcdef";while i<32 {
        let c=load8(hash+i);store8(text+i*2,load8(hex+(c>>4)));store8(text+i*2+1,load8(hex+(c&15)));i=i+1;
    }store8(text+64,0);return text;
}
fn puppy_step_clear() {
    if puppy_step_input {puppy_drop(puppy_step_input);puppy_step_input=0;}
    if puppy_step_catalog {puppy_drop(puppy_step_catalog);puppy_step_catalog=0;}
    if puppy_step_filename {puppy_drop(puppy_step_filename);puppy_step_filename=0;}
    if puppy_step_digest {puppy_drop(puppy_step_digest);puppy_step_digest=0;}
    if puppy_step_preview {puppy_drop(puppy_step_preview);puppy_step_preview=0;}
    puppy_step_index=0;puppy_step_replace=-1;puppy_step_has_tree=0;puppy_step_whole=0;return 1;
}
fn puppy_step_targets(project) {
    let targets=cg_array();let row=pc_first(j_get(project,"nodes"));while row {
        if m_eq(j_string(row,"type"),"faceted") {cg_push(targets,row);}row=load64(row+24);
    }return targets;
}
fn puppy_step_show(project) {
    if puppy_step_preview {puppy_drop(puppy_step_preview);puppy_step_preview=0;}
    let bodies=j_parse(puppy_step_catalog+24);let body=sr_arg(bodies,puppy_step_index);let id=j_integer(body,"id");
    let parsed=cad_step_read(puppy_step_input+24);let base=project;
    if !cad_project_is_studio(project) {base=0;}
    let action="Add";let label=m_cat(j_string(body,"name"),m_cat("   #",m_int(id)));let candidate=0;
    if parsed && puppy_step_whole {
        action="Import assembly";label="STEP assembly";
        candidate=cad_step_adopt(base,parsed,puppy_step_filename+24,puppy_step_digest+24);
        if candidate {label=j_string(j_get(candidate,"assembly"),"name");}
    }else if parsed {
        let target="";if puppy_step_replace>=0 {
            target=j_string(cg_at(puppy_step_targets(project),puppy_step_replace),"id");action=m_cat("Replace ",target);
        }
        let shape=cad_step_shape(parsed,id,puppy_step_filename+24,puppy_step_digest+24);
        if shape {candidate=cad_step_apply(base,shape,target);}
    }
    if !candidate || !puppy_refresh(candidate) {
        let reason=cg_error;if !reason {reason="Cannot display this body";}
        puppy_refresh(project);return puppy_title(project,m_cat(label,m_cat("   ",reason)));
    }
    puppy_step_preview=puppy_snapshot(j_encode(candidate));puppy_fit();
    return puppy_title(project,m_cat(action,m_cat("   ",label)));
}
fn puppy_step_begin(project) {
    let view=puppy_step_view;
    store64(view,puppy_yaw);store64(view+8,puppy_pitch);store64(view+16,puppy_roll);
    store64(view+24,puppy_zoom);m_copy(view+32,puppy_pan,24);
    store64(view+56,puppy_isolated);store64(view+64,puppy_isolated_owner);
    puppy_isolated=0;puppy_isolated_owner=0;return puppy_step_show(project);
}
fn puppy_step_finish(project,apply) {
    if puppy_button {puppy_view_end(0);}
    let view=puppy_step_view;let old=load64(view+56);let owner=load64(view+64);
    puppy_views_open=0;
    if apply {
        if old {puppy_drop(old);puppy_drop(owner);}
        let candidate=j_parse(puppy_step_preview+24);let status="STEP body imported";if puppy_step_whole {status="STEP assembly imported";}
        puppy_push(candidate);puppy_step_clear();return puppy_title(candidate,status);
    }
    puppy_step_clear();puppy_isolated=old;puppy_isolated_owner=owner;puppy_refresh(project);
    puppy_yaw=load64(view);puppy_pitch=load64(view+8);puppy_roll=load64(view+16);
    puppy_zoom=load64(view+24);m_copy(puppy_pan,view+32,24);return puppy_title(project,"STEP import cancelled");
}
fn puppy_step_prepare(path) {
    cg_begin();let source=puppy_read_limit(path,2097152);if !source {return cg_fail("Choose a readable STEP file up to 2 MiB");}
    let file=cad_step_read(source);let bodies=0;if file {bodies=cad_step_bodies(file);}
    if !bodies {return 0;}let hash=puppy_step_sha(source);if !hash {return 0;}
    puppy_step_clear();puppy_step_has_tree=cad_step_has_assembly(file);puppy_step_filename=puppy_snapshot(puppy_step_basename(path));puppy_step_digest=puppy_snapshot(hash);
    puppy_step_input=puppy_snapshot(source);puppy_step_catalog=puppy_snapshot(j_encode(bodies));
    return 1;
}
fn puppy_step_start(project,path) {
    if !m_len(path) {return puppy_title(project,"Enter a STEP file path");}
    if load8(path)!=47 {path=m_cat(m_slice(puppy_path,puppy_step_basename(puppy_path)-puppy_path),path);}
    if !puppy_step_prepare(path) {return puppy_title(project,cg_error);}
    if puppy_number_text {puppy_number_close();}return puppy_step_begin(project);
}
fn puppy_step_can_open(project) {
    return cad_project_is_studio(project) || (!puppy_disk && puppy_history==puppy_root && cad_project_part(project));
}
fn puppy_step_prompt(project) {
    if !puppy_step_can_open(project) {return puppy_title(project,"Open STEP into a new drawing or a standalone PCAD design");}
    puppy_step_path_entry=1;puppy_number_text=puppy_snapshot("");puppy_number_cursor=0;puppy_number_selected=0;
    gui_text_focus(load64(puppy_device),2);return puppy_title(project,"Open STEP file   |   Enter a path");
}
fn puppy_step_offer_assembly(project) {
    if !puppy_step_has_tree {return 0;}
    if cad_project_is_studio(project) {return cad_step_can_adopt(project);}
    return puppy_step_can_open(project);
}
fn puppy_step_actions(project) {
    let rows=cg_array();puppy_compact_button(rows,"Previous",GUI_KEY_LEFT,0);puppy_compact_button(rows,"Next",GUI_KEY_RIGHT,0);
    if puppy_step_offer_assembly(project) {puppy_compact_button(rows,"Assembly",4,0);}
    if !puppy_step_whole && cg_count(puppy_step_targets(project)) {puppy_compact_button(rows,"Target",23,0);}
    if puppy_step_preview {puppy_compact_button(rows,"Import",40,0);}puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);return rows;
}
fn puppy_step_key(project,key) {
    if key==GUI_KEY_ESCAPE {return puppy_step_finish(project,0);}
    if key==40 && puppy_step_preview {return puppy_step_finish(project,1);}
    if key==4 && puppy_step_offer_assembly(project) {puppy_step_whole=1-puppy_step_whole;return puppy_step_show(project);}
    if key==GUI_KEY_LEFT || key==GUI_KEY_RIGHT {
        puppy_step_whole=0;
        let count=cg_j_count(j_parse(puppy_step_catalog+24));let shift=1;if key==GUI_KEY_LEFT {shift=count-1;}
        puppy_step_index=(puppy_step_index+shift)%count;return puppy_step_show(project);
    }
    if key==23 && !puppy_step_whole {
        puppy_step_replace=puppy_step_replace+1;if puppy_step_replace>=cg_count(puppy_step_targets(project)) {puppy_step_replace=-1;}
        return puppy_step_show(project);
    }
    return 1;
}
fn puppy_step_event(event) {
    if !puppy_step_input {return 0;}let kind=gui_event_kind(event);let project=cad_project_open(puppy_history+24);
    if kind==GUI_QUIT || kind==GUI_CLOSE {puppy_step_key(project,GUI_KEY_ESCAPE);return 0;}
    if kind==GUI_KEY_DOWN {return puppy_step_key(project,gui_event_code(event));}
    if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 && gui_event_y(event)>=42 && gui_event_y(event)<68 {
        return puppy_action_bar(puppy_step_actions(project),0,gui_width(load64(puppy_device)),gui_event_x(event));
    }
    if puppy_step_preview {
        let button=gui_event_button(event);let x=gui_event_x(event);let y=gui_event_y(event);
        if kind==GUI_MOUSE_DOWN && y>=76 {
            if button==2 || button==3 {return puppy_view_down(button,x,y);}
            if button==1 {
                let label=m_slice(puppy_status+24,m_len(puppy_status+24));
                if puppy_cube_click(x,y) {return puppy_title(project,label);}
            }
        }
        if kind==GUI_MOUSE_MOVE && puppy_button {return puppy_view_move(x,y);}
        if puppy_button && (kind==GUI_MOUSE_UP || (kind==GUI_FOCUS && !gui_event_code(event))) {return puppy_view_end(0);}
        if kind==GUI_WHEEL {puppy_zoom=puppy_limit(puppy_zoom-gui_event_dy(event)*15,120,800);}
    }
    return kind!=GUI_RESIZE && kind!=GUI_EXPOSE;
}

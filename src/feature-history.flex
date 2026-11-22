import "cad/studio-history.flex";
import "cad/project-legacy-edit.flex";

// Rows contain saved feature IDs and the evaluator's status for this rebuild.
global puppy_history_rows=0;
global puppy_history_index=0;
global puppy_history_problem=0;
global puppy_history_part=0;
global puppy_history_preview=0;
global puppy_history_proposal=0;
global puppy_history_delta=0;
global puppy_history_recovery=0;
global puppy_history_legacy_index=0;
fn puppy_history_diagnostic(rows) {
    if puppy_history_problem {puppy_drop(puppy_history_problem);puppy_history_problem=0;}
    let row=pc_first(rows);
    while row {
        if m_eq(j_string(row,"state"),"error") {
            let name=j_string(row,"name");if !m_len(name) {name=j_string(row,"id");}
            puppy_history_problem=puppy_snapshot(m_cat(name," failed. Open History (F7) for details."));
            return 1;
        }
        row=load64(row+24);
    }
    return 1;
}
fn puppy_history_clear_preview() {
    if puppy_history_proposal {puppy_drop(puppy_history_proposal);puppy_history_proposal=0;}
    puppy_history_preview=0;puppy_history_delta=0;puppy_bar_page=0;
}
fn puppy_history_close() {
    if puppy_history_rows {puppy_drop(puppy_history_rows);puppy_history_rows=0;}
    if puppy_history_part {puppy_drop(puppy_history_part);puppy_history_part=0;}
    puppy_history_recovery=0;puppy_history_clear_preview();puppy_tree_dirty=1;return 1;
}
fn puppy_history_set_rows(rows) {
    if puppy_history_rows {puppy_drop(puppy_history_rows);}
    puppy_history_rows=puppy_snapshot(j_encode(rows));
}
fn puppy_history_feature(rows) {
    let row=pc_first(rows);let i=0;
    while row && i<puppy_history_index {row=load64(row+24);i=i+1;}
    return row;
}
fn puppy_history_legacy_field(project) {
    if !puppy_history_rows || !pc_legacy_solid_part(cad_project_part(project)) {return 0;}
    let row=puppy_history_feature(j_parse(puppy_history_rows+24));
    let fields=cad_project_legacy_step_fields(project,j_string(row,"id"));
    let count=cg_j_count(fields);if !count {return 0;}
    puppy_history_legacy_index=puppy_history_legacy_index%count;
    let field=sr_arg(fields,puppy_history_legacy_index);
    return cg_v(field,pc_legacy_number(field,"value",0),j_string(field,"label"));
}
fn puppy_history_select(project,rows) {
    let count=cg_j_count(rows);
    if !count {puppy_history_close();return puppy_title(project,"This part has no solid features");}
    puppy_history_index=puppy_limit(puppy_history_index,0,count-1);
    let row=puppy_history_feature(rows);
    let label=j_string(row,"name");if !m_len(label) {label=j_string(row,"id");}
    let state=j_string(row,"state");if m_eq(state,"rolledBack") {state="rolled back";}
    let text=m_cat(m_int(puppy_history_index+1),m_cat(" / ",m_int(count)));
    text=m_cat(text,m_cat("   ",m_cat(label,m_cat("   ",state))));
    let field=puppy_history_legacy_field(project);
    if field {text=m_cat(text,m_cat("   ",m_cat(load64(field+16),m_cat("  ",puppy_display_number(load64(field+8))))));}
    if j_get(row,"message") {text=m_cat(text,m_cat("   ",j_string(row,"message")));}
    return puppy_title(project,text);
}
fn puppy_history_begin_at(project,id,part) {
    let studio=cad_project_is_studio(project);let history=0;
    if studio {history=cad_studio_feature_history(project);}else {history=cad_project_feature_history(project);}
    let rows=j_get(history,"features");
    if !rows {return puppy_title(project,cg_error);}
    if !pc_first(rows) {return puppy_title(project,"This part has no solid features");}
    puppy_history_close();puppy_search_focus(0);puppy_picking_face=0;puppy_history_index=0;puppy_history_legacy_index=0;
    if studio {
        let first=pc_first(j_get(project,"parts"));
        if !part && first && !load64(first+24) {part=first;}
        if part {
            puppy_history_part=puppy_snapshot(j_string(part,"id"));
            if !m_len(id) {id=j_string(part,"node");}
        }
    }
    let row=pc_first(rows);let i=0;
    while row && !m_eq(j_string(row,"id"),id) {row=load64(row+24);i=i+1;}
    if row {puppy_history_index=i;}
    puppy_history_set_rows(rows);return puppy_history_select(project,rows);
}
fn puppy_history_begin(project) {
    let part=0;let id="";
    if cad_project_is_studio(project) {
        let row=puppy_design_selected(puppy_design_rows(project));
        if row {
            part=puppy_design_history_part(project,row);
            if m_eq(load64(row+16),"feature") {id=j_string(load64(row+24),"id");}
        }
    }
    return puppy_history_begin_at(project,id,part);
}
fn puppy_history_has_earlier(project,feature) {
    if !puppy_history_part {return 0;}
    if puppy_history_recovery && m_eq(j_string(feature,"state"),"error") {return 1;}
    let id=j_string(feature,"id");let earlier=pc_first(cad_studio_earlier_features(project,puppy_history_part+24));
    while earlier {
        if m_eq(j_value(earlier),id) {return 1;}
        earlier=load64(earlier+24);
    }
    return 0;
}
// Inspect only a known final modifier named by the evaluator. A previous
// state and the repaired full graph must independently produce closed solids.
fn puppy_history_try_recovery(project,reason) {
    if !reason || !cad_project_is_studio(project) {return 0;}
    let history=cad_studio_feature_history(project);let rows=j_get(history,"features");
    let part=pc_first(j_get(project,"parts"));
    while rows && part {
        let id=j_string(part,"node");let row=pc_find(rows,id);let source=j_string(row,"source");
        let prefix=m_cat("Feature ",m_cat(id,": "));let size=m_len(prefix);
        if m_len(source) && m_eq(j_string(row,"state"),"active") && m_len(reason)>size && m_eq(m_slice(reason,size),prefix) {
            let earlier=cad_studio_preview_feature(project,j_string(part,"id"),source);
            if j_value(j_get(earlier,"closed")) && cad_studio_suppress_feature(project,id,1) {
                puppy_history_begin_at(project,id,part);
                rows=j_parse(puppy_history_rows+24);row=pc_find(rows,id);
                cg_j_text(row,"state","error");cg_j_text(row,"message",reason+size);
                puppy_history_set_rows(rows);puppy_history_recovery=1;puppy_empty=1;
                return puppy_history_select(project,rows);
            }
        }
        part=load64(part+24);
    }
    cg_error=reason;return 0;
}
fn puppy_history_actions(project) {
    let rows=cg_array();
    if puppy_history_preview {
        if puppy_history_proposal {puppy_compact_button(rows,"Apply",40,0);}
        puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);return rows;
    }
    puppy_compact_button(rows,"Previous",80,0);puppy_compact_button(rows,"Next",79,0);
    let feature=puppy_history_feature(j_parse(puppy_history_rows+24));
    if pc_legacy_solid_part(cad_project_part(project)) {
        puppy_compact_button(rows,"Set value",59,0);puppy_compact_button(rows,"Next field",43,0);
        let operation="Join";
        let saved=j_string(feature,"operation");
        if m_eq(saved,"cut") {operation="Cut";}else if m_eq(saved,"intersect") {operation="Intersect";}
        puppy_compact_button(rows,operation,18,0);
        if m_eq(j_string(feature,"type"),"extrusion") {
            let data=j_get(cad_project_part(project),"data");let step=pc_find(pc_legacy_solid_steps(data),j_string(feature,"id"));
            let extent="Blind";if m_eq(j_string(step,"endCondition"),"through-all") {extent="Through all";}
            puppy_compact_button(rows,extent,8,0);
        }
        puppy_compact_button(rows,"Move earlier",82,0);puppy_compact_button(rows,"Move later",81,0);
        puppy_compact_button(rows,"Done",GUI_KEY_ESCAPE,0);return rows;
    }
    if cad_project_is_studio(project) {
        if j_get(feature,"source") {
            let toggle="Suppress";if m_eq(j_string(feature,"state"),"suppressed") {toggle="Restore";}
            puppy_compact_button(rows,toggle,22,0);
        }
        puppy_compact_button(rows,"Move earlier",82,0);puppy_compact_button(rows,"Move later",81,0);
        if puppy_history_has_earlier(project,feature) {puppy_compact_button(rows,"Preview earlier",4,0);}
        let finish="Done";if puppy_history_recovery {finish="Close file";}
        puppy_compact_button(rows,finish,GUI_KEY_ESCAPE,0);return rows;
    }
    let node=pc_find(pc_nodes(cad_project_part(project)),j_string(feature,"id"));let toggle="Suppress";
    if j_value(j_get(node,"suppressed")) {toggle="Unsuppress";}
    puppy_compact_button(rows,toggle,22,0);puppy_compact_button(rows,"Delete feature",76,0);
    puppy_compact_button(rows,"Roll before",5,0);
    puppy_compact_button(rows,"Roll after",4,0);puppy_compact_button(rows,"Restore end",8,0);
    puppy_compact_button(rows,"Move earlier",82,0);puppy_compact_button(rows,"Move later",81,0);
    puppy_compact_button(rows,"Done",GUI_KEY_ESCAPE,0);return rows;
}
fn puppy_history_studio_preview(project,row,key) {
    let id=j_string(row,"id");let proposal=0;let delta=0;let mesh=0;let status="";
    if key==4 {
        if !puppy_history_part {return puppy_title(project,"Open History from a part to preview its earlier states");}
        if puppy_history_recovery && m_eq(j_string(row,"state"),"error") {
            id=j_string(row,"source");row=pc_find(j_parse(puppy_history_rows+24),id);
        }
        mesh=cad_studio_preview_feature(project,puppy_history_part+24,id);
        status=m_cat("Earlier preview / ",j_string(row,"name"));
    }else {
        if key==22 {
            let suppress=!m_eq(j_string(row,"state"),"suppressed");
            proposal=cad_studio_suppress_feature(project,id,suppress);
            status="Restore preview / ";if suppress {status="Suppress preview / ";}
        }else {
            delta=1;if key==82 {delta=-1;}
            proposal=cad_studio_move_feature(project,id,delta);
            status="Move later preview / ";if delta<0 {status="Move earlier preview / ";}
        }
        if !proposal {return puppy_title(project,cg_error);}
        mesh=cad_project_mesh(proposal);status=m_cat(status,j_string(row,"name"));
    }
    if j_get(mesh,"error") {return puppy_title(project,cg_error);}
    if !puppy_upload(project,mesh) {
        let error=cg_error;if !error {error=gui_error;}
        return puppy_title(project,error);
    }
    puppy_empty=0;puppy_clear_pick();puppy_history_preview=1;puppy_history_delta=delta;puppy_bar_page=0;
    if proposal {puppy_history_proposal=puppy_snapshot(j_encode(proposal));}
    return puppy_title(project,status);
}
fn puppy_history_finish_preview(project,key) {
    if key==GUI_KEY_ESCAPE || key==64 {
        if puppy_history_recovery {puppy_empty=1;}
        else if !puppy_refresh(project) {return puppy_title(project,cg_error);}
        puppy_history_clear_preview();return puppy_title(project,"Feature preview cancelled");
    }
    if key!=40 || !puppy_history_proposal {return 1;}
    let updated=cad_project_open(puppy_history_proposal+24);
    if !puppy_refresh(updated) {return puppy_title(project,cg_error);}
    puppy_push(updated);puppy_history_index=puppy_history_index+puppy_history_delta;
    puppy_history_clear_preview();return puppy_history_select(updated,j_parse(puppy_history_rows+24));
}
fn puppy_history_legacy_key(project,row,key) {
    let id=j_string(row,"id");
    if key==59 {let field=puppy_history_legacy_field(project);if field {return puppy_number_open(project,field);}return 1;}
    if key==43 {puppy_history_legacy_index=puppy_history_legacy_index+1;return puppy_history_select(project,j_parse(puppy_history_rows+24));}
    let updated=0;let shift=0;
    if key==82 || key==81 {
        shift=1;if key==82 {shift=-1;}
        updated=cad_project_move_legacy_step(project,id,shift);
    }else if key==18 {
        let operation=j_string(row,"operation");let next="join";
        if m_eq(operation,"join") {next="cut";}else if m_eq(operation,"cut") {next="intersect";}
        updated=cad_project_edit_legacy_step(project,id,"operation",j_text(next));
    }else if key==8 && m_eq(j_string(row,"type"),"extrusion") {
        let data=j_get(cad_project_part(project),"data");let step=pc_find(pc_legacy_solid_steps(data),id);
        let next="through-all";if m_eq(j_string(step,"endCondition"),"through-all") {next="blind";}
        updated=cad_project_edit_legacy_step(project,id,"endCondition",j_text(next));
    }else {return 1;}
    if !updated || !puppy_refresh(updated) {return puppy_title(project,cg_error);}
    puppy_push(updated);puppy_history_index=puppy_history_index+shift;
    return puppy_history_select(updated,j_parse(puppy_history_rows+24));
}
fn puppy_history_key(project,key) {
    if puppy_history_preview {return puppy_history_finish_preview(project,key);}
    if puppy_history_recovery && (key==GUI_KEY_ESCAPE || key==64) {return key==64;}
    if key==GUI_KEY_ESCAPE || key==64 {puppy_history_close();if puppy_history_problem {return puppy_title(project,puppy_history_problem+24);}return puppy_title(project,"Feature history closed");}
    let rows=j_parse(puppy_history_rows+24);let count=cg_j_count(rows);
    if key==80 || key==79 {
        if key==80 {puppy_history_index=puppy_history_index-1;}else {puppy_history_index=puppy_history_index+1;}
        puppy_history_legacy_index=0;
        return puppy_history_select(project,rows);
    }
    if pc_legacy_solid_part(cad_project_part(project)) {return puppy_history_legacy_key(project,puppy_history_feature(rows),key);}
    if cad_project_is_studio(project) {
        if key==4 || key==22 || key==81 || key==82 {return puppy_history_studio_preview(project,puppy_history_feature(rows),key);}
        return 1;
    }
    let updated=0;let move=0;let removed=0;
    if key==22 || key==76 || key==81 || key==82 {
        let row=puppy_history_feature(rows);
        let id=j_string(row,"id");let node=pc_find(pc_nodes(cad_project_part(project)),id);
        if key==22 {updated=cad_project_suppress_feature(project,id,!j_value(j_get(node,"suppressed")));}
        else if key==76 {
            removed=j_string(row,"name");if !m_len(removed) {removed=id;}
            updated=cad_project_delete_feature(project,id);
        }
        else {
            move=1;if key==82 {move=-1;}
            updated=cad_project_move_feature(project,id,move);
        }
    }else if key==4 || key==5 || key==8 {
        let limit=puppy_history_index;if key==4 {limit=limit+1;}if key==8 {limit=count;}
        updated=cad_project_roll_to(project,limit);
    }else {return 1;}
    if !updated {return puppy_title(project,cg_error);}
    if !m_eq(j_encode(updated),puppy_history+24) {
        if !puppy_refresh(updated) {return puppy_title(project,cg_error);}
        puppy_push(updated);puppy_history_index=puppy_history_index+move;
    }
    rows=j_parse(puppy_history_rows+24);
    if removed && !pc_first(rows) {
        puppy_history_close();puppy_design_select_item(cad_project_part(updated));
        return puppy_title(updated,m_cat("Deleted ",removed));
    }
    return puppy_history_select(updated,rows);
}

fn puppy_history_offset(count,height) {
    let visible=puppy_limit((height-12)/32,1,512);
    return puppy_limit(puppy_history_index-visible+1,0,puppy_limit(count-visible,0,count));
}
fn puppy_history_panel(w,h) {
    let rows=j_parse(puppy_history_rows+24);let row=pc_first(rows);let index=0;
    let first=puppy_history_offset(cg_j_count(rows),h);let y=6;
    let rgb=m_take(w*h*4);puppy_ui_rect(rgb,w,h,0,0,w,h,32,45,49);
    while row && y<h {
        if index>=first {
            if index==puppy_history_index {puppy_ui_rect(rgb,w,h,5,y,w-10,29,54,77,75);}
            let name=j_string(row,"name");if !m_len(name) {name=j_string(row,"id");}
            let state=j_string(row,"state");if m_eq(state,"rolledBack") {state="rolled back";}
            puppy_ui_text(rgb,w,h,12,y+20,name,w-99);puppy_ui_text(rgb,w,h,w-94,y+20,state,w-7);
            y=y+32;
        }
        row=load64(row+24);index=index+1;
    }
    puppy_ui_upload(rgb,w,h);puppy_tree_width=w;puppy_tree_height=h;puppy_tree_dirty=0;return 1;
}
fn puppy_history_click(project,y) {
    if puppy_history_preview {return 1;}
    let rows=j_parse(puppy_history_rows+24);let count=cg_j_count(rows);
    let index=puppy_history_offset(count,gui_height(load64(puppy_device))-76)+(y-82)/32;
    if y>=82 && index<count {puppy_history_index=index;puppy_history_legacy_index=0;return puppy_history_select(project,rows);}
    return 1;
}

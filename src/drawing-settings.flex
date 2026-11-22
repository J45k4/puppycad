global puppy_sheet_setting=-1;
global puppy_sheet_entry=0;

fn puppy_sheet_setting_fields(project) {
    let id=j_string(j_parse(puppy_sheet_scene+24),"id");
    let sheet=pc_find(j_get(project,"drawings"),id);
    let fields=cg_array();
    cg_push(fields,cg_v("name","Sheet name",j_string(sheet,"name")));
    let detail=j_get(sheet,"detail");
    if detail {
        cg_push(fields,cg_v("magnification","Magnification (0 for Auto)",m_int(j_integer(detail,"magnification"))));
    }else {
        cg_push(fields,cg_v("axis","Orientation (top, front, right, iso)",j_string(sheet,"axis")));
        cg_push(fields,cg_v("scale","Scale denominator (0 for Auto)",m_int(j_integer(sheet,"scale"))));
        if !j_get(sheet,"exploded_offsets") {
            cg_push(fields,cg_v("target","Occurrence ID (blank for entire model)",j_string(sheet,"target")));
            let section=j_get(sheet,"section_mm");let depth="";
            if section {depth=puppy_display_number(cg_number(section));}
            cg_push(fields,cg_v("section_mm","Section depth in mm (blank for projection)",depth));
        }
        if !j_get(sheet,"section_mm") {
            let edges=j_string(sheet,"visibility");
            if !m_len(edges) {edges="wire";}
            cg_push(fields,cg_v("visibility","Edges (wire, visible, hidden)",edges));
        }
    }
    return fields;
}

fn puppy_sheet_setting_prompt(project) {
    let fields=puppy_sheet_setting_fields(project);
    let count=cg_count(fields);
    puppy_sheet_setting=(puppy_sheet_setting+count)%count;
    let field=cg_at(fields,puppy_sheet_setting);
    let value=load64(field+16);
    if !m_len(value) {value="—";}
    return puppy_title(project,m_cat(load64(field+8),m_cat("   ",value)));
}

fn puppy_sheet_apply_setting(project,id,settings,message) {
    let result=cad_drawing_edit_sheet(project,id,settings);
    if !result {return puppy_title(project,cg_error);}
    let updated=j_get(result,"document");
    if !m_eq(j_encode(updated),j_encode(project)) {
        puppy_push(updated);
        puppy_exit_requested=0;
    }
    let scene=j_get(result,"scene");
    if puppy_number_text {puppy_number_close();}
    puppy_sheet_show(scene);
    return puppy_title(updated,m_cat(message,j_string(scene,"name")));
}

fn puppy_sheet_add(project) {
    puppy_sheet_setting=-1;
    puppy_search_focus(0);puppy_query_set("");
    return puppy_sheet_apply_setting(project,"",j_object(),"Added drawing sheet  ");
}

fn puppy_sheet_entry_apply(project,text) {
    let settings=j_object();
    let field=cg_at(puppy_sheet_setting_fields(project),puppy_sheet_setting);
    let key=load64(field);
    if m_eq(key,"scale") || m_eq(key,"magnification") {
        let value=m_number(text);
        if !m_eq(m_int(value),text) {return puppy_title(project,"Enter a whole number");}
        cg_j_int(settings,key,value);
    }else if m_eq(key,"section_mm") {
        if m_len(text) {
            cg_begin();
            let value=j_parse(text);
            if j_kind(value)!=1 && j_kind(value)!=6 {return puppy_title(project,"Enter a signed decimal depth");}
            j_add(settings,key,value);
        }else {j_add(settings,key,j_new(0,0));}
    }else {cg_j_text(settings,key,text);}
    let id=j_string(j_parse(puppy_sheet_scene+24),"id");
    return puppy_sheet_apply_setting(project,id,settings,"Updated drawing sheet  ");
}

fn puppy_sheet_history(project,next) {
    if !next {return 1;}
    let id=j_string(j_parse(puppy_sheet_scene+24),"id");
    let updated=cad_project_open(next+24);
    if !puppy_refresh(updated) {puppy_refresh(project);return puppy_title(project,"Could not restore drawing");}
    puppy_history=next;
    puppy_sheet_close();
    if pc_find(j_get(updated,"drawings"),id) {return puppy_sheet_open(updated,id);}
    return puppy_title(updated,"Model view");
}

fn puppy_sheet_key(project,key,mods) {
    if mods&192 {
        if key==19 {return puppy_export_shortcut(project,".pdf");}
        if key==22 {return puppy_save(project);}
        if key==29 && !(mods&3) {return puppy_sheet_history(project,load64(puppy_history));}
        if key==28 || key==29 {return puppy_sheet_history(project,load64(puppy_history+8));}
        return 1;
    }
    if key==GUI_KEY_ESCAPE {
        if puppy_sheet_setting>=0 {
            puppy_sheet_setting=-1;
            return puppy_title(project,m_cat("Drawing sheet  ",j_string(j_parse(puppy_sheet_scene+24),"name")));
        }
        puppy_sheet_close();
        return puppy_title(project,"Model view");
    }
    if puppy_sheet_setting<0 && key==59 {
        puppy_design_focus=0;
        puppy_sheet_setting=0;
        return puppy_sheet_setting_prompt(project);
    }
    if puppy_sheet_setting<0 {return 1;}
    if key==80 || key==79 || key==43 {
        if key==80 {puppy_sheet_setting=puppy_sheet_setting-1;}
        else {puppy_sheet_setting=puppy_sheet_setting+1;}
        return puppy_sheet_setting_prompt(project);
    }
    if key==40 || key==59 {
        let field=cg_at(puppy_sheet_setting_fields(project),puppy_sheet_setting);
        puppy_sheet_entry=1;
        return puppy_text_open(project,load64(field+16));
    }
    return 1;
}

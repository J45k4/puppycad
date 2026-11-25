// Inline rename for Design tree records and the open .pcad project file.
global puppy_rename_mode=0;
global puppy_rename_label=0;
global puppy_path_owned=0;

fn puppy_rename_close() {
    puppy_rename_mode=0;
    if puppy_rename_label {puppy_drop(puppy_rename_label);puppy_rename_label=0;}
    return 1;
}
fn puppy_rename_drop_path() {
    if puppy_path_owned {puppy_drop(puppy_path_owned);puppy_path_owned=0;}
    return 1;
}
fn puppy_rename_trim(text) {
    let end=m_len(text);
    while end && load8(text+end-1)<=32 {end=end-1;}
    while end && load8(text)<=32 {text=text+1;end=end-1;}
    return m_slice(text,end);
}
fn puppy_path_base(path) {
    let base=path;let i=0;
    while load8(path+i) {if load8(path+i)==47 {base=path+i+1;}i=i+1;}
    return base;
}
fn puppy_path_dir(path) {
    let base=puppy_path_base(path);
    return m_slice(path,base-path);
}
fn puppy_path_stem(path) {
    let base=puppy_path_base(path);let n=m_len(base);
    if n>=5 && m_eq(base+n-5,".pcad") {n=n-5;}
    return m_slice(base,n);
}
fn puppy_design_renameable(row) {
    if !row || !load64(row+24) {return 0;}
    let kind=load64(row+16);
    return m_eq(kind,"part") || m_eq(kind,"assembly") || m_eq(kind,"sketch")
        || m_eq(kind,"extrude") || m_eq(kind,"revolve") || m_eq(kind,"loft")
        || m_eq(kind,"edgeFinish") || m_eq(kind,"chamfer") || m_eq(kind,"instance")
        || m_eq(kind,"mate") || m_eq(kind,"definition") || m_eq(kind,"subassembly")
        || m_eq(kind,"feature") || m_eq(kind,"sheet");
}
fn puppy_rename_prompt_text() {
    if puppy_rename_label {return puppy_rename_label+24;}
    if puppy_rename_mode==2 {return "Rename file";}
    return "Rename";
}
fn puppy_rename_item_begin(project,row) {
    if !puppy_design_renameable(row) {return puppy_title(project,"Select an item to rename");}
    puppy_rename_close();
    let label=m_cat("Rename ",load64(row));
    puppy_rename_mode=1;puppy_rename_label=puppy_snapshot(label);
    puppy_text_open(project,load64(row));
    return puppy_title(project,label);
}
fn puppy_rename_file_begin(project) {
    if puppy_reader_report {return 1;}
    puppy_rename_close();
    puppy_rename_mode=2;puppy_rename_label=puppy_snapshot("Rename file (.pcad)");
    puppy_text_open(project,puppy_path_base(puppy_path));
    return puppy_title(project,"Rename file");
}
fn puppy_rename_item_apply(project,text) {
    let name=puppy_rename_trim(text);
    if !m_len(name) {return puppy_title(project,"Enter a name");}
    let row=puppy_design_selected(puppy_design_rows(project));
    if !puppy_design_renameable(row) {puppy_number_close();return puppy_title(project,"Select an item to rename");}
    let node=load64(row+24);
    if m_eq(j_string(node,"name"),name) && m_eq(load64(row),name) {
        puppy_number_close();return puppy_title(project,"Name unchanged");
    }
    cg_j_text(node,"name",name);
    puppy_push(project);
    puppy_number_close();puppy_tree_dirty=1;puppy_toolbar_dirty=1;
    return puppy_title(project,m_cat("Renamed to ",name));
}
fn puppy_rename_file_apply(project,text) {
    let name=puppy_rename_trim(text);let n=m_len(name);
    if !n || m_eq(name,".") || m_eq(name,"..") || load8(name+n-1)==47 {
        return puppy_title(project,"Enter a file name");
    }
    if n<5 || !m_eq(name+n-5,".pcad") {name=m_cat(name,".pcad");n=m_len(name);}
    let has_slash=0;let i=0;
    while i<n {if load8(name+i)==47 {has_slash=1;}i=i+1;}
    let target=name;
    if !has_slash {target=m_cat(puppy_path_dir(puppy_path),name);}
    let base=puppy_path_base(target);
    if !m_len(base) || m_eq(base,".pcad") || m_eq(base,"..pcad") {
        return puppy_title(project,"Enter a file name");
    }
    if m_eq(target,puppy_path) {
        puppy_number_close();return puppy_title(project,"File name unchanged");
    }
    if puppy_disk && !m_eq(puppy_read(puppy_path),puppy_disk+24) {
        return puppy_title(project,"File changed on disk");
    }
    if !cad_project_is_studio(project) && puppy_revision==0x7fffffffffffffff {
        return puppy_title(project,"Revision limit reached");
    }
    let old_stem=puppy_path_stem(puppy_path);let new_stem=puppy_path_stem(target);
    let updated_part=0;
    if !cad_project_is_studio(project) {
        let def=pc_find(j_get(project,"items"),"native-part-1");
        if def && m_eq(j_string(def,"name"),old_stem) && m_len(new_stem) {
            cg_j_text(def,"name",new_stem);updated_part=1;
        }
        cg_j_int(project,"revision",puppy_revision+1);
    }
    let encoded=j_encode(project);
    let staged=puppy_stage(target,encoded,m_len(encoded));
    if !staged {return puppy_title(project,"Rename failed");}
    let status=syscall(316,-100,staged,-100,target,1,0);
    if status {
        syscall(87,staged,0,0,0,0,0);
        if status==-17 {return puppy_title(project,"File already exists");}
        return puppy_title(project,"Rename failed");
    }
    if puppy_disk {syscall(87,puppy_path,0,0,0,0,0);}
    let next_path=puppy_snapshot(target);
    puppy_rename_drop_path();puppy_path_owned=next_path;puppy_path=next_path+24;
    if updated_part {puppy_push(project);}
    if puppy_disk {puppy_drop(puppy_disk);}
    puppy_disk=puppy_snapshot(encoded);puppy_revision=puppy_revision+1;
    puppy_saved_history=puppy_history;puppy_projection_save_needed=0;
    puppy_number_close();puppy_tree_dirty=1;puppy_toolbar_dirty=1;
    return puppy_title(project,m_cat("Renamed to ",puppy_path_base(puppy_path)));
}
fn puppy_rename_apply(project,text) {
    if puppy_rename_mode==2 {return puppy_rename_file_apply(project,text);}
    return puppy_rename_item_apply(project,text);
}

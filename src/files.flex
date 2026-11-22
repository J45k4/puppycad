fn puppy_read(path) {return puppy_read_limit(path,0);}
fn puppy_read_limit(path,limit) {
    let fd=syscall(2,path,0x80000,0,0,0,0);
    if fd<0 {return 0;}
    let buffer=m_buffer();let scratch=m_take(4096);let n=1;
    while n>0 || n==-4 {
        n=syscall(0,fd,scratch,4096,0,0,0);
        if n>0 {
            if limit && load64(buffer+8)+n>limit {n=-1;}
            else {
                let i=0;while i<n && load8(scratch+i) {i=i+1;}
                if i<n {n=-1;}else {m_append(buffer,scratch,n);}
            }
        }
    }
    syscall(3,fd,0,0,0,0,0);
    if n<0 {return 0;}return m_data(buffer);
}
// Write beside the destination so a failed export cannot truncate an existing drawing.
fn puppy_stage(path,bytes,size) {
    let temporary=m_cat(path,m_cat(".tmp-",m_int(syscall(39,0,0,0,0,0,0))));
    let stat=m_take(144);
    let mode=420;
    if syscall(4,path,stat,0,0,0,0)==0 {mode=load64(stat+24)&511;}
    let fd=syscall(2,temporary,0x800c1,mode,0,0,0);
    if fd<0 {return 0;}
    let at=0;let ok=1;
    while ok && at<size {
        let written=syscall(1,fd,bytes+at,size-at,0,0,0);
        if written>0 {at=at+written;}else if written!=-4 {ok=0;}
    }
    if syscall(3,fd,0,0,0,0,0)!=0 {ok=0;}
    if ok {return temporary;}
    syscall(87,temporary,0,0,0,0,0);return 0;
}

fn puppy_write(path,bytes,size) {
    let temp=puppy_stage(path,bytes,size);
    if !temp {return 0;}
    if syscall(82,temp,path,0,0,0,0)==0 {return 1;}
    syscall(87,temp,0,0,0,0,0);return 0;
}
fn puppy_instance_stem(instance) {
    let label=j_string(instance,"name");if !m_len(label) {label=j_string(instance,"id");}
    let b=m_buffer();let i=0;let end=0;
    while load8(label+i) {
        let c=load8(label+i);
        if (c>=48 && c<=57) || (c>=65 && c<=90) || (c>=97 && c<=122) || c==95 {
            m_append(b,label+i,1);end=load64(b+8);
        }else if end && (c<128 || c>=192) {m_text(b,"-");}
        i=i+1;
    }
    if !end {return "instance";}return m_slice(m_data(b),end);
}
fn puppy_prepare_exports(project,path,fresh) {
    let length=m_len(path);
    let dot=length;let k=0;
    while k<length {
        let c=load8(path+k);if c==46 {dot=k;}else if c==47 {dot=length;}k=k+1;
    }
    let extension=path+dot;let format=m_slice(extension,length-dot);k=1;
    while k<length-dot {let c=load8(format+k);if c>=65 && c<=90 {store8(format+k,c+32);}k=k+1;}
    let bodies=0;let instance=0;
    if m_eq(format,".3mf") || m_eq(format,".step") || m_eq(format,".stp") {
        let file=0;
        if m_eq(format,".3mf") {file=cad_project_3mf(project);}else {file=cad_project_step(project);}
        if !file {return 0;}bodies=cg_array();if !cg_push(bodies,file) {return 0;}
    }else if m_eq(format,".stl") {
        let mesh=cad_project_mesh(project);if j_get(mesh,"error") {return 0;}
        bodies=cad_project_stls(mesh);if !bodies {return 0;}
        let item=cad_project_item(project);
        if cad_project_is_studio(project) && j_get(project,"assembly") {instance=pc_first(j_get(mesh,"parts"));}
        if m_eq(j_string(item,"type"),"assembly") {instance=pc_first(j_get(j_get(item,"data"),"instances"));}
    }else {return cg_fail("Select a .stl, .3mf, .step or .stp output");}
    if !cg_count(bodies) {return cg_fail("No bodies to export");}
    let base=m_slice(path,dot);
    let files=m_take(8+cg_count(bodies)*24);store64(files,cg_count(bodies));
    let i=0;let suffix=1;
    while i<cg_count(bodies) {
        let file=cg_at(bodies,i);let name=path;
        let stem=base;if instance {stem=m_cat(base,m_cat("-",puppy_instance_stem(instance)));name=m_cat(stem,extension);}
        if suffix>1 {name=m_cat(stem,m_cat("-",m_cat(m_int(suffix),extension)));}
        suffix=suffix+1;
        let info=m_take(144);let status=syscall(6,name,info,0,0,0,0);
        let duplicate=0;let j=0;
        while instance && j<i && !duplicate {duplicate=m_eq(load64(files+8+j*24),name);j=j+1;}
        if !duplicate && (!fresh || status!=0) {
            let staged=0;
            if status==-2 || (status==0 && (load64(info+24)&0xf000)==0x8000) {
                staged=puppy_stage(name,load64(file),load64(file+8));
            }
            if !staged {
                let j=0;while j<i {syscall(87,load64(files+16+j*24),0,0,0,0,0);j=j+1;}
                return cg_fail("Cannot stage export file");
            }
            let entry=files+8+i*24;
            store64(entry,name);store64(entry+8,staged);store64(entry+16,0);i=i+1;
            if instance {instance=load64(instance+24);suffix=1;}
        }
    }
    return files;
}
fn puppy_export(project,path,fresh) {
    let files=puppy_prepare_exports(project,path,fresh);
    if !files {return 0;}
    let ok=1;let i=0;
    while ok && i<load64(files) {
        let entry=files+8+i*24;let dest=load64(entry);let temp=load64(entry+8);
        // renameat2: publish a new file exclusively, or exchange an existing
        // output with its staging file until all bodies have been published.
        let rc=syscall(316,-100,temp,-100,dest,1,0);
        if rc==0 {store64(entry+16,1);}
        else if !fresh && rc==-17 && syscall(316,-100,temp,-100,dest,2,0)==0 {store64(entry+16,2);}
        else {ok=0;}
        i=i+1;
    }
    i=load64(files);
    while i {
        i=i-1;let entry=files+8+i*24;let state=load64(entry+16);let temp=load64(entry+8);
        if !ok && state==1 {syscall(87,load64(entry),0,0,0,0,0);}
        if !ok && state==2 && syscall(316,-100,temp,-100,load64(entry),2,0)!=0 {
            return cg_fail("Export rollback failed; prior output remains in its staging file");
        }
        if state!=1 {syscall(87,temp,0,0,0,0,0);}
    }
    if !ok {return cg_fail("Cannot publish export file");}
    return load64(files+8);
}

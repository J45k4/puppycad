import "cad/sketch-dimension.flex";
import "cad/threemf.flex";
import "cad/step.flex";
import "cad/drawings.flex";
import "cad/author.flex";
import "cad/project-access.flex";
import "cad/studio-edit.flex";
import "src/files.flex";
import "src/editor.flex";

fn puppy_error(message) {m_print(2,m_cat("PuppyCAD: ",m_cat(message,"\n")));return 1;}

fn main(argc,argv) {
    if argc==2 {
        let path=load64(argv+8);let ext=puppy_step_extension(path);
        if ext>=0 {
            let mark=m_mark();if !puppy_step_prepare(path) {return puppy_error(cg_error);}m_reset(mark);
            let stem=m_slice(path,ext);let destination=m_cat(stem,".pcad");let n=2;let stat=m_take(144);
            while !syscall(6,destination,stat,0,0,0,0) {destination=m_cat(stem,m_cat("-",m_cat(m_int(n),".pcad")));n=n+1;}
            return puppy_editor(destination,0,cad_project_new(puppy_step_basename(stem)),0);
        }
        let source=puppy_read(path);
        if !source {
            if syscall(6,path,m_take(144),0,0,0,0)!=-2 {return puppy_error("Cannot read project file");}
            let start=0;let end=m_len(path);let i=0;
            while i<end {if load8(path+i)==47 {start=i+1;}i=i+1;}
            if end-start>=5 && m_eq(path+end-5,".pcad") {end=end-5;}
            let name="Untitled";if end>start {name=m_slice(path+start,end-start);}
            return puppy_editor(path,0,cad_project_new(name),0);
        }
        let report=cad_project_inspect_source(source);
        if !report {return puppy_error(cg_error);}
        return puppy_editor(path,source,j_get(report,"document"),report);
    }
    if argc==3 && m_eq(load64(argv+8),"--step-bodies") || (argc>=4 && argc<=6) && m_eq(load64(argv+8),"--import-step") {
        let path=load64(argv+16);let source=puppy_read_limit(path,2097152);if !source {return puppy_error("Choose a readable STEP file up to 2 MiB");}
        let file=cad_step_read(source);if !file {return puppy_error(cg_error);}let bodies=cad_step_bodies(file);if !bodies {return puppy_error(cg_error);}
        if argc==3 {m_print(1,j_encode(bodies));m_print(1,"\n");return 0;}
        let digest=puppy_step_sha(source);if !digest {return puppy_error(cg_error);}
        let filename=puppy_step_basename(path);let project=0;
        if argc==5 {
            if !m_eq(load64(argv+32),"--assembly") {return puppy_error("Expected --assembly or --body ID");}
            project=cad_step_assembly(file,filename,digest);
        }else {
            let body=pc_first(bodies);let id=j_integer(body,"id");
            if argc==6 {
                if !m_eq(load64(argv+32),"--body") {return puppy_error("Expected --body ID");}
                let number=load64(argv+40);id=m_number(number);if id<=0 || !m_eq(m_int(id),number) {return puppy_error("Invalid STEP body ID");}
            }else if load64(body+24) {return puppy_error("Choose a solid with --body ID from --step-bodies or use --assembly");}
            let shape=cad_step_shape(file,id,filename,digest);if shape {project=cad_step_apply(0,shape,0);}
        }
        if !project {return puppy_error(cg_error);}
        let output=load64(argv+24);let bytes=j_encode(project);let temp=puppy_stage(output,bytes,m_len(bytes));
        if !temp {return puppy_error("Cannot write imported drawing");}
        if syscall(316,-100,temp,-100,output,1,0) {syscall(87,temp,0,0,0,0,0);return puppy_error("Choose a new output file");}return 0;
    }
    if argc!=4 && argc!=6 {return puppy_error("Usage: puppycad FILE.pcad | FILE.step\n       puppycad --step-bodies INPUT.step\n       puppycad --import-step INPUT.step NEW.pcad [--body ID | --assembly]\n       puppycad --roundtrip INPUT.pcad OUTPUT.pcad\n       puppycad --export INPUT.pcad OUTPUT [--assembly NAME]");}
    let command=load64(argv+8);let source=puppy_read(load64(argv+16));let output=load64(argv+24);
    if !source {return puppy_error("Cannot open input file");}
    if argc==4 && m_eq(command,"--roundtrip") {
        let report=cad_project_inspect_source(source);
        if !report {return puppy_error(cg_error);}
        let bytes=source;
        if j_get(report,"readOnly") {
            let input_stat=m_take(144);let output_stat=m_take(144);
            if syscall(4,load64(argv+16),input_stat,0,0,0,0)==0 && syscall(4,output,output_stat,0,0,0,0)==0
                && load64(input_stat)==load64(output_stat) && load64(input_stat+8)==load64(output_stat+8) {return 0;}
        }else {bytes=j_encode(j_get(report,"document"));}
        if !puppy_write(output,bytes,m_len(bytes)) {return puppy_error("Cannot write output");}
        return 0;
    }
    let project=cad_project_open(source);
    if !project {return puppy_error(cg_error);}
    if argc==6 {
        if !m_eq(command,"--export") || !m_eq(load64(argv+32),"--assembly") {return puppy_error("Expected --export INPUT OUTPUT --assembly NAME");}
        let wanted=load64(argv+40);let row=pc_first(j_get(project,"items"));let index=0;let path=j_array();
        while row && !pc_first(path) {
            if m_eq(j_string(row,"type"),"assembly") && (m_eq(j_string(row,"id"),wanted) || m_eq(j_string(row,"name"),wanted)) {j_add(path,0,j_number(index));}
            row=load64(row+24);index=index+1;
        }
        if !pc_first(path) {return puppy_error("Assembly not found");}cg_j_set(project,"selectedPath",path);
    }
    if !m_eq(command,"--export") {return puppy_error("Unknown command");}
    if !puppy_export(project,output,0) {return puppy_error(cg_error);}
    return 0;
}

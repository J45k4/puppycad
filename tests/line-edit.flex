import "cad/project-offset-author.flex";

fn main(argc,argv) {
    if argc<4 {return 2;}
    let project=cad_project_open(m_read_file(load64(argv+8)));if !project {return 1;}
    let before=j_encode(project);let sketch=load64(argv+16);let operation=load64(argv+24);let result=0;
    if m_eq(operation,"inspect") {result=cad_project_line_sketch(project,sketch);}
    else if argc==6 {
        let id=load64(argv+32);let value=j_parse(load64(argv+40));
        if m_eq(operation,"offset") {result=cad_project_add_offset(project,sketch,id,j_value(value));}
        else if m_eq(operation,"length") {result=cad_project_set_line_length(project,sketch,id,pc_pose_number(value));}
        else if m_eq(operation,"center") {result=cad_project_move_point(project,sketch,id,value);}
        else if m_eq(operation,"distance") {result=cad_project_set_distance_expression(project,sketch,id,j_value(value));}
        else if m_eq(operation,"reference") {result=cad_project_toggle_distance_reference(project,sketch,id);}
        else if m_eq(operation,"variable") {result=cad_project_edit_line_variable(project,sketch,id,j_string(value,"name"),j_string(value,"expression"));}
        else if m_eq(operation,"expression") {result=cad_project_set_line_expression(project,sketch,id,j_value(value));}
        else {result=cad_project_move_line_endpoint(project,sketch,id,operation,value);}
    }else {return 2;}
    m_assert(m_eq(j_encode(project),before),"Line edit mutated its input");
    if !result {m_print(2,cg_error);return 1;}
    m_print(1,j_encode(result));return 0;
}

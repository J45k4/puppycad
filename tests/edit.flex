// Drive the public library on the same input loaded by the editor.
import "cad/sketch-dimension.flex";
import "cad/project-access.flex";
fn main(argc,argv) {
    if argc==2 {
        let report=cad_project_inspect_source(m_read_file(load64(argv+8)));
        if !report || j_get(report,"readOnly") {return 1;}return 0;
    }
    if argc!=5 {return 2;}
    let project=cad_project_open(m_read_file(load64(argv+8)));
    if !project {return 1;}
    let before=j_encode(project);
    let mesh=cad_project_dimension(project,load64(argv+16),load64(argv+24),j_parse(load64(argv+32)));
    if !mesh {
        m_assert(m_eq(before,j_encode(project)),"Rejected edit changed the graph");
        m_print(2,cg_error);return 1;
    }
    m_print(1,j_encode(project));return 0;
}

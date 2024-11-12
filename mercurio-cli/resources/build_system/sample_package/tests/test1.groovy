import io.github.petrotta.mercurio.testing.SysMLTest;
import io.github.petrotta.mercurio.build.Project;

import org.eclipse.emf.ecore.EObject;
import org.eclipse.emf.ecore.resource.Resource;
import org.omg.sysml.lang.sysml.Element;

class X1 {

    @SysMLTest
    void test1(Project prj) {
        System.out.println("prj: " + prj)

    }


    @SysMLTest
    void test2(Project prj) {
        System.out.println("prj: " + prj)
        System.out.println(prj.getInputResources());

        for(Resource r : prj.getInputResources()) {
            for(Element e : r.getAllContents()) {
                System.out.println("..." + e.getName());
            }


        }
    }

}



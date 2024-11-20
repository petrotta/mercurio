import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Assertions;
import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.Application;

import org.eclipse.emf.ecore.resource.Resource;
import org.omg.sysml.lang.sysml.Element;




class X1 {

    @Test
    void test1() {
        def prj = Application.getCurrentProject();

        System.out.println("prj: " + prj)
        Assertions.assertTrue(prj != null)
    }


    @Test
    void test2() {
        def prj = Application.getCurrentProject();
        
        System.out.println("prj: " + prj)
        System.out.println(prj.getInputResources());

        for(Resource r : prj.getInputResources()) {
            for(Element e : r.getAllContents()) {
                System.out.println("..." + e.getName());
            }


        }

    }

}



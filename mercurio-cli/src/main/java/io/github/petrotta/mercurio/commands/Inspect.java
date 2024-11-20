package io.github.petrotta.mercurio.commands;

import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;
import org.eclipse.emf.ecore.EObject;
import org.eclipse.emf.ecore.resource.Resource;
import org.omg.sysml.lang.sysml.Element;
import picocli.CommandLine;

import java.io.File;
import java.util.concurrent.Callable;

import static io.github.petrotta.mercurio.Application.console;

@CommandLine.Command(name = "inspect", mixinStandardHelpOptions = true,
        description = "Examines source code.")
public class Inspect extends ProjectCommand implements Callable<Integer> {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;

    @Override
    public Integer call() throws Exception { // your business logic goes here...

        Project project = Application.openProject(sourceDir, libDir, verbose);
        project.readSysML();

        for(Resource r : project.getInputResources()) {
            System.out.println(r.getURI());
            for(EObject e : r.getContents()) {
                printContents(e, 0);
            }

        }
        Application.console("# resources read: " + project.getResourceSet().getResources().size());


        return 0;
    }



    void printContents(EObject object, int level) {
        for(int i=0;i<level;i++) {System.out.print(" ");}

        if(object instanceof Element) {
            Element element = (Element) object;
            if(element.getName()!=null) {
                System.out.print(element.getName());
            }
            System.out.print(" : " );
            System.out.println(element.eClass().getName());

            for(EObject e : object.eContents()) {
                printContents(e, level+1);
            }
        }


    }

}
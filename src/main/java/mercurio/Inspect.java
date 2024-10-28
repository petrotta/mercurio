package mercurio;

import org.eclipse.emf.ecore.EObject;
import org.eclipse.emf.ecore.resource.Resource;
import org.eclipse.xtext.validation.Issue;
import org.omg.sysml.interactive.SysMLInteractive;
import org.omg.sysml.lang.sysml.Element;
import picocli.CommandLine;

import java.io.File;
import java.util.List;
import java.util.concurrent.Callable;

@CommandLine.Command(name = "inspect", mixinStandardHelpOptions = true,
        description = "Examines source code.")
class Inspect implements Callable<Integer> {

    @CommandLine.Option(names = { "-src", "--src" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @Override
    public Integer call() throws Exception { // your business logic goes here...


        File userDir = new File(System.getProperty("user.dir"));
        //File curDir = null;
        if(sourceDir == null) {
            sourceDir = userDir;
            //curDir = userDir;
        }


        System.out.println("User Dir: " + userDir.getAbsolutePath());
        System.out.println("Source Dir:   "+ sourceDir.toString());

        if(libDir == null) {
            libDir = Application.getLibraryDir();
            System.out.println("Library Dir (default): " + libDir.getAbsolutePath());
        } else {
            System.out.println("Library Dir: " + libDir.getAbsolutePath());
        }

        SysMLInteractive interactive = Application.getSysMLInteractive();

        interactive.readAll(sourceDir.toString(),true, ".sysml");

        for(Resource r : interactive.getInputResources()) {
            System.out.println(r.getURI());
            for(EObject e : r.getContents()) {
                printContents(e, 0);
            }

        }
//        interactive.validate();

        System.out.println("# resources read: " + interactive.getResourceSet().getResources().size());

//        List<Issue> issues = interactive.validate();
//        System.out.println("total issues: " + issues.size());
//        for(Issue issue : issues) {
//            System.out.println(issue);
//        }

        return 0;
    }

//    public static void main(String[] args) {
//        int exitCode = new CommandLine(new Validate()).execute(args);
//    }

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
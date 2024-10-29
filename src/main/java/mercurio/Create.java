package mercurio;

import org.eclipse.emf.ecore.EObject;
import org.eclipse.emf.ecore.resource.Resource;
import org.omg.sysml.interactive.SysMLInteractive;
import picocli.CommandLine;

import java.io.File;
import java.io.PrintWriter;
import java.util.concurrent.Callable;

@CommandLine.Command(name = "create", mixinStandardHelpOptions = true,
        description = "Creates an empty sysml package.")
public class Create implements Callable<Integer>  {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;


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

        if(!sourceDir.exists()) {
            sourceDir.mkdir();

            File srcDir = new File(sourceDir, "src");
            srcDir.mkdir();

            File sysmlFile = new File(srcDir, "Example.sysml");
            sysmlFile.createNewFile();

            //new File(sourceDir, "build").mkdir();
            File packageFile = new File(sourceDir, "package.xml");
            packageFile.createNewFile();
            {
                PrintWriter printWriter = new PrintWriter(packageFile);
                printWriter.println("<package>");
                printWriter.println("   <org>ABS</org>");
                printWriter.println("   <project>Simple SysML system</project>");
                printWriter.println("   <version>1.0.0</version>");

                printWriter.println("</package>");
                printWriter.close();
            }
        } else {
            System.out.println("Directory already exists.");
        }



        return 0;
    }

//    public static void main(String[] args) {
//        int exitCode = new CommandLine(new Validate()).execute(args);
//    }
}

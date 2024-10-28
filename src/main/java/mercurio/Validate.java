package mercurio;

import org.eclipse.xtext.validation.Issue;
import org.omg.sysml.interactive.SysMLInteractive;
import picocli.CommandLine;

import java.io.File;
import java.util.List;
import java.util.concurrent.Callable;

@CommandLine.Command(name = "validate", mixinStandardHelpOptions = true,
        description = "Checks the source files for compile errors.")
class Validate implements Callable<Integer> {

    @CommandLine.Option(names = { "-src", "--src" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;

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
        //System.out.println("Lib Dir:     "+ libDir.toString());
        SysMLInteractive interactive = Application.getSysMLInteractive();

        interactive.readAll(sourceDir.toString(),true, ".sysml");
        interactive.validate();

        System.out.println("# resources read: " + interactive.getResourceSet().getResources().size());

        List<Issue> issues = interactive.validate();
        System.out.println("total issues: " + issues.size());
        for(Issue issue : issues) {
            System.out.println(issue);
        }

        return 0;
    }

    public static void main(String[] args) {
        int exitCode = new CommandLine(new Validate()).execute(args);
    }


}
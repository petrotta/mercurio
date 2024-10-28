package mercurio;

import org.eclipse.xtext.validation.Issue;
import org.omg.sysml.interactive.SysMLInteractive;
import picocli.CommandLine;

import java.io.File;
import java.util.List;
import java.util.concurrent.Callable;

@CommandLine.Command(name = "inspect", mixinStandardHelpOptions = true,
        description = "Examines source code.")
class Inspect implements Callable<Integer> {

    @CommandLine.Option(names = { "-src", "--src" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File file;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @Override
    public Integer call() throws Exception { // your business logic goes here...


        File userDir = new File(System.getProperty("user.dir"));
        File curDir = null;
        if(file == null) {
            curDir = userDir;
        }
        System.out.println("Working Dir: " + curDir);
        System.out.println("Start Dir:   "+ file.toString());

        System.out.println("Lib Dir:     "+ libDir.toString());
        SysMLInteractive interactive = Application.getSysMLInteractive();

        interactive.readAll(file.toString(),true, ".sysml");
        interactive.validate();

        System.out.println("# resources read: " + interactive.getResourceSet().getResources().size());

        List<Issue> issues = interactive.validate();
        System.out.println("total issues: " + issues.size());
        for(Issue issue : issues) {
            System.out.println(issue);
        }

        return 0;
    }

//    public static void main(String[] args) {
//        int exitCode = new CommandLine(new Validate()).execute(args);
//    }


}
import org.eclipse.xtext.validation.Issue;
import org.omg.sysml.interactive.SysMLInteractive;
import picocli.CommandLine;

import java.io.File;
import java.util.List;
import java.util.concurrent.Callable;

@CommandLine.Command(name = "validate", mixinStandardHelpOptions = true,
        description = "Checks the source files for compile errors.")
class Validate implements Callable<Integer> {

    @CommandLine.Option(names = { "-src", "--src" }, paramLabel = "SOURCE", description = "the source files or directories")
    private File file;


    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory")
    //@CommandLine.Parameters(index = "0", description = "The file or directory to check.")
    private File libDir;


//    @CommandLine.Option(names = {"-a", "--algorithm"}, description = "MD5, SHA-1, SHA-256, ...")
//    private String algorithm = "SHA-256";

    @Override
    public Integer call() throws Exception { // your business logic goes here...

        System.out.println("Working Directory = " + System.getProperty("user.dir"));
        System.out.println("file: "+ file.toString());

        SysMLInteractive interactive = SysMLInteractive.createInstance();

        if(libDir != null) {
            interactive.readAll(new File(libDir, "Kernal Libraries").getPath(), false, ".kerml");
            interactive.readAll(new File(libDir, "Systems Libraries").getPath(), false, ".sysml");
            interactive.readAll(new File(libDir, "Domain Libraries").getPath(), false, ".sysml");
        }
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

    public static void main(String[] args) {
        int exitCode = new CommandLine(new Validate()).execute(args);
    }


}
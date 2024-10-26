import org.eclipse.xtext.validation.Issue;
import org.omg.sysml.interactive.SysMLInteractive;
import picocli.CommandLine;

import java.io.File;
import java.util.List;
import java.util.concurrent.Callable;

@CommandLine.Command(name = "eval", mixinStandardHelpOptions = true,
        description = "Compile the source code.")
public class Evaluate implements Callable<Integer> {

    @CommandLine.Option(names = { "-src", "--src" }, paramLabel = "SOURCE", description = "the source files or directories")
    private File file;


    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory")
    //@CommandLine.Parameters(index = "0", description = "The file or directory to check.")
    private File libDir;

    @CommandLine.Option(names = { "-i", "--input" }, paramLabel = "INPUT", description = "eval input")
    private String input;

    @CommandLine.Option(names = { "-t", "--target" }, paramLabel = "TARGET", description = "eval target")
    private String target;

    @Override
    public Integer call() throws Exception {
        System.out.println("Working Directory = " + System.getProperty("user.dir"));
        System.out.println("file: "+ file.toString());

        SysMLInteractive interactive = SysMLInteractive.createInstance();

        if(libDir != null) {
            interactive.readAll(new File(libDir, "Kernal Libraries").getPath(), false, ".kerml");
            interactive.readAll(new File(libDir, "Systems Libraries").getPath(), false, ".sysml");
            interactive.readAll(new File(libDir, "Domain Libraries").getPath(), false, ".sysml");
        }
        interactive.readAll(file.toString(),true, ".sysml");


        System.out.println("# resources read: " + interactive.getResourceSet().getResources().size());


        System.out.println( interactive.eval(input, target));

        return 0;
    }
}

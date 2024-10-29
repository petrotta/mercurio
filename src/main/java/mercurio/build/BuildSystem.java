package mercurio.build;

import org.apache.commons.digester.Digester;
import org.xml.sax.SAXException;



import java.io.File;
import java.io.IOException;

public class BuildSystem {


     public static PackageManifest readBuildFile(File file) throws IOException, SAXException {

         Digester digester = new Digester();

         // Create Employee object when encountering <employee> tag
         digester.addObjectCreate("package", PackageManifest.class);

         // Set properties of Employee object
         digester.addBeanPropertySetter("package/project", "project");
         digester.addBeanPropertySetter("package/org", "org");
         digester.addBeanPropertySetter("package/version", "version");

         digester.addObjectCreate("package/depend", String.class);
         digester.addSetProperties("package/depend");
         digester.addBeanPropertySetter("package/depend", "text");
         digester.addSetNext("package/depend", "addDepend");

//         digester.addObjectCreate("package/depend", StringBuffer.class);
//         digester.addCallMethod("package/depend", "addDepend", 1);
         PackageManifest pkg = (PackageManifest) digester.parse(file);

         return pkg;



    }
}

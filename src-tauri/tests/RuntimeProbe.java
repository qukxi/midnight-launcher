public class RuntimeProbe {
  public static void main(String[] args) throws Exception {
    Class.forName("net.minecraft.client.main.Main", false, RuntimeProbe.class.getClassLoader());
    String lwjgl = org.lwjgl.Version.getVersion();
    String glfw = org.lwjgl.glfw.GLFW.glfwGetVersionString();
    System.out.println("Minecraft main class found; Java " + System.getProperty("java.version") + " " + System.getProperty("os.arch") + "; LWJGL " + lwjgl + "; GLFW " + glfw);
  }
}

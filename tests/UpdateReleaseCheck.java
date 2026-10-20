package org.example;

/** Release assets may contain a source ZIP before the Windows application ZIP. */
public final class UpdateReleaseCheck {
    public static void main(String[] args) {
        if (!"1.9.1".equals(Version.VERSION) || !"0.6.7.7".equals(Version.LAB_VERSION))
            throw new AssertionError("Kit and Lab versions must remain independent");
        if (!UpdateChecker.isNewer("0.6.7.8", Main.LAB_VERSION))
            throw new AssertionError("Application updates must use the Lab version");
        if (!UpdateChecker.isNewer(Main.LAB_VERSION, "0.6.5")
                || !UpdateChecker.isNewer(Main.LAB_VERSION, "0.6.7")
                || UpdateChecker.isNewer("0.6.7", Main.LAB_VERSION)
                || UpdateChecker.isNewer(Main.LAB_VERSION, Main.LAB_VERSION))
            throw new AssertionError("Four-part Lab versions must compare numerically");
        String source = "{\"name\":\"gekitotsu_lab-0.6.7.7-source.zip\",\"browser_download_url\":\"https://example.invalid/source.zip\"}";
        String windows = "{\"name\":\"gekitotsu_lab-0.6.7.7-windows-x64.zip\",\"browser_download_url\":\"https://example.invalid/windows.zip\"}";
        String legacy = "{\"name\":\"GekitotsuKit v1.9.1.zip\",\"browser_download_url\":\"https://example.invalid/legacy.zip\"}";
        UpdateChecker.Release selected = UpdateChecker.parseRelease("{\"tag_name\":\"v0.6.7.7\",\"assets\":[" + source + "," + legacy + "," + windows + "]}");
        if (selected == null || !"https://example.invalid/windows.zip".equals(selected.zipUrl()))
            throw new AssertionError("The Windows application ZIP must take priority");
        selected = UpdateChecker.parseRelease("{\"tag_name\":\"v0.6.7.7\",\"assets\":[" + source + "]}");
        if (selected == null || selected.zipUrl() != null)
            throw new AssertionError("A source ZIP must never be offered as an application update");
        System.out.println("发布附件选择通过：Windows便携包优先，源码包不作为自动更新。");
    }
}

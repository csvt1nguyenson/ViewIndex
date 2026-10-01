Set WshShell = CreateObject("WScript.Shell")
WshShell.CurrentDirectory = "D:\YouTubeMetadataAnalyzer"
WshShell.Run "YouTubeMetadataAnalyzer.exe", 0, False

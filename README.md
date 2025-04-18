# AssandQrcConverter-rust

相互转换带有 K 值 `{\kxx}` 的 ASS 字幕文件（也被称为**卡拉 OK**）和 QRC 歌词文件。  

在转换完成后，请**检查**输出文件是否正确。

建议搭配 [Aegisub](https://github.com/TypesettingTools/Aegisub) 和 [163MusicLyrics](https://github.com/jitwxs/163MusicLyrics) 取得最佳效果。
## 示例
原始文件

>[29264,3446]故(29264,390)事(29654,392)的(30046,448)小(30494,922)黄(31416,374)花(31790,504)  
[32710,3537]从(32710,452)出(33162,380)生(33542,537)那(34079,482)年(34561,183)就(34744,271)飘(35015,392)着(35407,434)  
[36247,3505]童(36247,456)年(36703,425)的(37128,440)荡(37568,856)秋(38424,392)千(38816,535)  
[39752,3165]随(39752,871)记(40623,217)忆(40840,245)一(41085,208)直(41293,225)晃(41518,209)到(41727,238)现(41965,409)在(42374,312)  
[42917,3017]Re (42917,313)So (43230,440)So (43670,440)Si (44110,448)Do (44558,456)Si (45014,440)La (45454,256)  
[45934,3940]So (45934,184)La (46118,241)Si (46359,382)Si (46741,425)Si (47166,432)Si (47598,539)La (48137,184)Si (48321,204)La (48525,422)So (48947,503)  



转换结果

>[Events]  
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text  
Dialogue: 0,0:00:29.26,0:00:32.71,Default,,0,0,0,,{\k39}故{\k39}事{\k45}的{\k92}小{\k37}黄{\k50}花{\k42}  
Dialogue: 0,0:00:32.71,0:00:36.24,Default,,0,0,0,,{\k45}从{\k38}出{\k54}生{\k48}那{\k18}年{\k27}就{\k39}飘{\k43}着{\k41}  
Dialogue: 0,0:00:36.24,0:00:39.75,Default,,0,0,0,,{\k46}童{\k43}年{\k44}的{\k86}荡{\k39}秋{\k54}千{\k40}  
Dialogue: 0,0:00:39.75,0:00:42.91,Default,,0,0,0,,{\k87}随{\k22}记{\k25}忆{\k21}一{\k23}直{\k21}晃{\k24}到{\k41}现{\k31}在{\k23}  
Dialogue: 0,0:00:42.91,0:00:45.93,Default,,0,0,0,,{\k31}Re {\k44}So {\k44}So {\k45}Si {\k46}Do {\k44}Si {\k26}La {\k22}  
Dialogue: 0,0:00:45.93,0:00:49.87,Default,,0,0,0,,{\k18}So {\k24}La {\k38}Si {\k43}Si {\k43}Si {\k54}Si {\k18}La {\k20}Si {\k42}La {\k50}So {\k42}  

## 注意事项
- ASS 字幕格式的时间戳只能精确到 10 毫秒，但 QRC 的时间戳可以精确到 1 毫秒。将 QRC 转换为 ASS 时，QRC 的时间戳会被四舍五入，这会导致转换后的 ASS 文件时间轴与原始 QRC 文件存在偏差。建议使用 [Aegisub](https://github.com/TypesettingTools/Aegisub) 校对时间轴。

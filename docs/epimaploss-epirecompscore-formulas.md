# EpiMapLoss / EpiRecompScore

## 日本語

このページは、開発中のEpistesysが、変換で何を失ったか、候補の再編にどんな価値があるかをどう表すかを説明します。数式の基準はBeta 6.9で公開した[固定測定契約](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md)と[設計revision](../validation/tlcx/design-revision-chain-2026-10-29.json)の **finite-formal-v1.4** です。

**読み方：** [写像保存損失](#map-ja) → [再編評価のF1・F2と下位式](#score-ja) → [全40成分](#catalog-ja)。実装や数値検証に使う場合は[共通計算](#kernels-ja)・[選択と探索](#policy-ja)・[数値例](#examples-ja)も参照してください。

`EpiMapLoss`（写像保存損失）と`EpiRecompScore`（構造再編評価値）は、この資料で使う現在の公開名です。元の式ID・数学記号・既存のRust／JSON互換名は変更していません。新しいRust型やwire fieldを追加したという意味ではありません。

**記号：** $z$は候補、$z_0$は同じ比較範囲の基準候補です。$P$は事前に固定した検査課題（probe）の集合、$w_i>0$は重みです。以下の式は、比較範囲、基準、適用軸、単位、検証方法、予算が一致するときに使います。`Exact`は与えられた入力に対する算術上の正確さです。

<a id="map-ja"></a>

### EpiMapLoss：写像で失われたもの

元の入力 $X$ を表現 $Y_i$ に写すとき、保存すべき性質を検査します。再構成・意味・時間・空間が四つの軸です。原案は軸と写像を示していましたが、軸を一つに集約する式は定めていません。以下は現在の操作的測定規約です。

$$
X\xrightarrow{F_i}Y_i,\qquad \mathcal A\subseteq\{\mathrm{recon},\mathrm{sem},\mathrm{time},\mathrm{space}\}.
$$

$$
\begin{aligned}
W_a &= \sum_{q\in P_a}w_q,\qquad
L_a=\left[\frac{\sum_q w_q\underline\ell_q}{W_a},\frac{\sum_q w_q\overline\ell_q}{W_a}\right],\\
\operatorname{EpiMapLoss}_{\mathcal A,\rho}(z)
&=L_{\mathrm{trans}}(z)=\frac{\sum_{a\in\mathcal A}\rho_a L_a(z)}{\sum_{a\in\mathcal A}\rho_a},\qquad \rho_a>0 .
\end{aligned}
$$

関係を保ったと確認できればプローブ損失は0、違反を確認すれば1です。測る量と値域が定義され、観測だけが不足する場合には $[0,1]$ を保持できます。定義不能・比較不能・未対応を0で埋めません。軸・分母を候補ごとに変えず、必須条件の違反を平均で埋め合わせません。

$$
e_q=\begin{cases}
\lvert v_t-F_q(v_s)\rvert&\text{registered scalar},\\
\lVert v_t-F_q(v_s)\rVert_{\text{registered norm}}&\text{registered vector norm},
\end{cases}
\qquad
\ell_q=\min\!\left(1,\frac{\max(0,e_q-\epsilon_q)}{r_q}\right).
$$

許容差 $\epsilon_q\ge0$ と飽和尺度 $r_q>0$ は誤差と同じ単位です。空間では登録された比較器を使います。時間では時点と経過時間を分けます。非空区間は $s<e$ とし、区間内の時点は $s\le t<e$ です。beforeは $e_A<s_B$、meetsは $e_A=s_B$ です。

$$
F_{\mathrm{abs}}(t)=at+b,\qquad F_{\mathrm{dur}}(\Delta t)=a\,\Delta t.
$$

限定された[現行Rust実装](../scripts/lc631/crates/lc631-runtime/src/checked_tl_reduction.rs)の一例は、時間と空間の二軸、重み1:3を固定した有限MODELの比較です。この係数をすべての課題の既定値とみなしません。多段写像の終端損失は元の性質を最終表現で測り直した値であり、経路の損失の単純和ではありません。

$$
L_{\mathrm{tuple}}=\frac{L_{\mathrm{time}}+3L_{\mathrm{space}}}{4},\qquad L_{\mathrm{end}}=L(X,Y_{\mathrm{final}};\,P_{\mathrm{root}}).
$$

一回の変換には四つの記録 $(D,\Delta,U,G)$ を付けます。$D$は確認済みの保存義務違反、$\Delta$は許容した前提変更、$U$は未確定事項、$G$は基準に対する利得の記録です。これらは四つの数を足す式ではありません。意図した前提変更は欠陥と区別し、予測した利得と観測した利得も区別します。

<a id="score-ja"></a>

### EpiRecompScore：F1・F2と全下位式

**F1**（固定ID `OriginalFormula/F1-v1`）。文脈・意味配置からの距離、既存体系への接続、複雑性の釣り合いを評価し、写像保存損失と既知の繰返しを差し引きます。$\operatorname{EpiRecompScore}_{F1}(z)=S_1(z)$ です。

$$
\begin{aligned}
S_1(z)&=\alpha D_{\mathrm{ctx}}(z)+\beta D_{\mathrm{sem}}(z) \\
&\quad -\lambda L_{\mathrm{trans}}(z)+\gamma A_{\mathrm{compat}}(z) \\
&\quad +\delta C_{\mathrm{match}}(z)-\eta N_{\mathrm{known}}(z).
\end{aligned}
$$

**F2**（固定ID `OriginalFormula/F2-v1`）。新規性と接続性、構造利得、再編、実用利得、複雑性の釣り合いを評価し、写像保存損失、反例リスク、接続コストを差し引きます。$\operatorname{EpiRecompScore}_{F2}(z)=S_2(z)$ です。

$$
\begin{aligned}
S_2(z)&=w_1T_{\mathrm{nc}}(z)+w_2G_{\mathrm{struct}}(z) \\
&\quad +w_3R_{\mathrm{gain}}(z)+w_4U_{\mathrm{gain}}(z) \\
&\quad +w_5Q_{\mathrm{parity}}(z)-w_6L_{\mathrm{trans}}(z) \\
&\quad -w_7R_{\mathrm{counter}}(z)-w_8B_{\mathrm{mem}}(z).
\end{aligned}
$$

**新規性と接続性の両立**

$$
T_{\mathrm{nc}}(z)=\sqrt{D_{\mathrm{par}}(z)D_{\mathrm{sem}}(z)}
\,A_{\mathrm{compat}}(z).
$$

**構造利得**

$$
G_{\mathrm{struct}}=
a_1\Delta coverage+a_2\Delta compression+a_3\Delta bridge
+a_4\Delta predict-a_5Redundancy.
$$

**既存構造の保存と再編**

$$
R_{\mathrm{gain}}=
b_1Preserve+b_2Rebind+b_3Reframe-b_4BreakCost.
$$

**実用利得**

$$
U_{\mathrm{gain}}=
c_1\Delta explain+c_2\Delta design+c_3\Delta transfer+c_4\Delta solve.
$$

**複雑性の釣り合い**

$$
Q_{\mathrm{parity}}(z)=
\exp\!\left(-\frac{|K(z)-K_0|}{\sigma}\right).
$$

F1の $D_{\rm ctx}$ とF2の $D_{\rm par}$ は別の量です。$C_{\rm match}$ と $Q_{\rm parity}$ も別です。創造側の $D_{\rm sem}$ は任意の意味配置の差で、必須の意味を失う $L_{\rm sem}$ ではありません。四記録の $U$ と $U_{\rm gain}$、四記録の $G$ と $G_{\rm struct}$ を混同しません。

$$
\Delta S_j=S_j(z)-S_j(z_0),\qquad j\in\{1,2\}.
$$

生の得点と基準からの差は別に報告します。恒等候補でも生の得点は一般に0ではありません。原式の係数は27個（F1: 6、F2: 8、構造: 5、再編: 4、実用: 4）。規約では有限非負有理数を使い、係数和1は原式の要件ではありません。$\sigma,\kappa_M$ は正のbits、$\kappa_B$ は正の接続コスト単位です。均等係数は試験用で、校正済みの本番重みではありません。

以下は係数が非負の場合の外側の値域です。均等係数は `reference-uniform-test-v1` の例で、接続コストの尺度 $\kappa_B$ は試験ごとに指定します。

$$
\begin{aligned}
S_1&\in[-(\lambda+\eta),\,\alpha+\beta+\gamma+\delta],\\
G_{\rm struct}&\in[-\sum_{i=1}^{5}a_i,\,\sum_{i=1}^{4}a_i]=[g_l,g_h],\\
R_{\rm gain}&\in[-b_4,\,b_1+b_2+b_3]=[r_l,r_h],\qquad
U_{\rm gain}\in[-\sum_{i=1}^{4}c_i,\,\sum_{i=1}^{4}c_i]=[u_l,u_h],\\
S_2&\in[w_2g_l+w_3r_l+w_4u_l-w_6-w_7-w_8,\;w_1+w_2g_h+w_3r_h+w_4u_h+w_5].
\end{aligned}
$$

$$
(\alpha,\beta,\lambda,\gamma,\delta,\eta)=(1/6,\ldots,1/6),\qquad
w_i=1/8,\quad a_i=1/5,\quad b_i=c_i=1/4,\quad \sigma=\kappa_M=K_0.
$$

[Rustの式評価器](../scripts/lc631/crates/lc631-analysis/src/creativity_measurement/formula/evaluate.rs)は、参照成分が欠けていれば0で補いません。係数が0でも未定義の入力を救済せず、厳密なF1/F2値を作れないときは`NoScalar`です。[実行経路の事前検査](../scripts/lc631/crates/lc631-runtime/src/original_score_pending.rs)は、入力が揃わない場合に評価済みスコアではなくPendingを残します。算術の実装、各成分の生成、通常経路の完了、独立した効果の実証は別です。

<a id="catalog-ja"></a>

### 全40成分：定義・値域・算出法

以下は同じrevisionの成分カタログです。表記上 $\operatorname{Avg}$ は固定重み平均、$\operatorname{Pair}$ は同じ課題で比較した基準からの差、$d_J$ は重み付き集合距離です。指示関数は条件が確認されたときだけ0または1になり、未観測から0を作りません。記録集合にはスカラー式を捏造しません。各成分の適用条件・独立検証・選択への影響は[規範契約](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md)にあります。

<details>

<summary>写像保存損失と補助量 (7)</summary>

#### `TL-RECON` · reconstruction loss

**定義・値域・単位:** 許可された逆写像H(Y)で元の保護featureを再構成できない割合。固定P_reconにK1を適用。L_recon∈[0,1]、無次元。byte一致率とは別量。

**算出法:** target semantic payloadだけを隔離decoderへ渡す→再構成→固定feature oracleで照合→各失敗1/成功0/未観測[0,1]→K1。decoderの辞書・model・補助情報はpinし、Kの圧縮costにも含める。

#### `TL-SEM` · semantic preservation loss

**定義・値域・単位:** 固定必須意味probeの保存関係に違反する重み割合。各probeはequivalence/soundness/completenessのいずれかを明記。K1、[0,1]、無次元。創造側D_semとは異なる。

**算出法:** source/target queryを固定mappingでlowerし、各Γについて先にconsistencyを検査する。contextがSATならΓ∧¬φがUNSATのときだけφをSupported、Γ∧φがUNSATのときだけRefuted、両方SATならUndetermined。equivalenceは両向きの含意queryがともにSupported、soundnessは各target-claimedφ_tの逆写像がsourceでSupported、completenessは各Required source-claimedφ_sの写像がtargetでSupportedとなることを検査する。ΓがUNSATならContextInconsistent、結果がConflict/Unknown/timeout/Unsupportedならlossを確定せずUnknown等のreasonを保持する。違反を独立確認したprobeのみ1、完全passのみ0。

#### `TL-TIME` · temporal preservation loss

**定義・値域・単位:** 固定時間probeの順序・duration・絶対時刻等の保存違反。二値関係は0/1。数値probeは`e_q=|v_target−F_q(v_source)|`（同一canonical unit）から`ell=min(1,max(0,e_q−epsilon_q)/r_q)`を算出する。r_q>0と許容差epsilon_q>=0は同一unit。K1により[0,1]無次元へ集約。

**算出法:** ClockId/TimeScale/Origin/Unit/Precision/ReferenceIdを検査し、まず共通canonical unitへ厳密換算する。Fはcanonical source time→canonical target time。absolute instantは`F_abs(t)=a*t+b`、elapsed durationは`F_dur(Δt)=a*Δt`でbを加えない。intervalはstart<endの非空`[start,end)`、逆転/空intervalはInvalidInput、瞬間eventは別型。beforeはendA<startB、meetsはendA=startB。order/duration/absolute timestampは別probe。誤差区間の後にepsilon/r_q式で外向き計算。

#### `TL-SPACE` · spatial preservation loss

**定義・値域・単位:** 固定空間probeのtopology/包含/方向/metric保存違反。関係は0/1。数値probeはComparatorIdで固定した`e_q=ComparatorId(F(source),target)`から`ell=min(1,max(0,e_q−epsilon_q)/r_q)`を算出する。epsilon_q>=0とr_q>0は同じunit。K1後は[0,1]無次元。

**算出法:** FrameId/次元/軸/origin/unit/handedness/object対応/TransformClassを検査。許可写像後、ComparatorIdで距離・角度・包含等のerrorを評価。vector NormId、角度wrap rule、epsilon、r_qはpolicy revisionで固定し、未登録ならUnsupported。変換/測定誤差の区間を伝播してからlossへ写す。

#### `TL-TOTAL` · EpiMapLoss / L_trans

**定義・値域・単位:** L_trans=Σ_(a∈A)ρ_a L_a/Σρ_a、ρ_a>0、Aは比較前固定の適用軸。値域[0,1]、無次元。原案には集約式がなく本規約で補完。

**算出法:** 元contextからaxis maskと重みを固定→各軸の有効性確認→K1と同様に区間加重和。W=0はUndefined。N/A軸はscope作成時のみ除外し、候補ごとには除外しない。

#### `TL-PATH` · end-to-end preservation

**定義・値域・単位:** 元source→最終targetの同じ保護probeを直接再検査したL_end∈[0,1]。edge lossの単純和ではない。D ledgerはunique(root obligation, property, revision)で欠陥を同定。

**算出法:** 各edgeに対応写像とwitnessを保存→map compositionを検査→最終targetでTL各軸を再計測→同一欠陥をroot keyでまとめる。重複した1欠陥を複数lossとして加算しない。

#### `TL-MODAL` · modal_likeness

**定義・値域・単位:** 固定required capability集合に対して実際に対応可能と確認した重み割合、K1のsuccess版、[0,1]無次元。真理確率でない補助適合度。

**算出法:** registryの対応宣言と該当revisionの実行probeの両方を照合。宣言のみはunobserved。成功1、確認不対応0、不明[0,1]を固定分母で平均。

</details>

<details>

<summary>変換ごとの四記録 (4)</summary>

#### `REC-D` · four-record Defect

**定義・値域・単位:** Dは保護義務に反した事象の有限集合でありscalarでない。key=(root obligation, property, source/target revision)、各事象に対応TL軸と根拠。countのunitは件だがcount=TLではない。

**算出法:** 保存probeの確認違反からのみ生成→root keyでdedup→span/transform/observationsへリンク。未観測はDに確定せずREC-Uへ。

#### `REC-DELTA` · four-record permitted change

**定義・値域・単位:** Deltaは元／候補context、変更対象、変更前後、理由、許可根拠、map-backを持つtyped edit集合。件数は距離/利得そのものではない。

**算出法:** 可変premise/frameworkへの差分を抽出し、hard setとの交差を拒否→各論理における変更意味を記録→Dctx/Dpar/Dsemへ入力。元contextは不変。

#### `REC-U` · four-record uncertainty

**定義・値域・単位:** Uは未観測/曖昧/比較不能/未対応/未検証のgap集合。確率やU_gainでない。件数とscopeは報告可能だが1個のconfidenceに縮約しない。

**算出法:** 各metric/probeの未確定状態とreasonをunique keyで記録し、必要証拠と次actionを保持。状態の違いをfalseへ潰さない。

#### `REC-G` · four-record gain

**定義・値域・単位:** Gは固定baseline相対の検証された保存/再結合/能力変化のevent集合。各eventはmetric、primitive feature、paired observationを参照。数式G_structとは別。

**算出法:** 観測gainと予測gainを別tagで登録→shared primitive IDで重複を検出→CR各項へ集計。削除された必須条件をgainにしない。

</details>

<details>

<summary>文脈・新規性・接続性 (7)</summary>

#### `CR-DCTX` · D_ctx

$$
D_{\mathrm{ctx}}=d_J(P_0^{\mathrm{mutable}},P_z^{\mathrm{mutable}})
$$

**定義・値域・単位:** 元contextと再構成contextの可変premise/fact feature集合のK3距離。[0,1]無次元。logicの公理/推論規則signatureはD_parに分離。

**算出法:** scopeにpinしたuniverseで両集合をcanonical化→差集合/和集合の重み和をK3計算。変更不能featureの差は先にhard violation。

#### `CR-DPAR` · D_par

$$
D_{\mathrm{par}}=d_J(\Sigma_0,\Sigma_z)
$$

**定義・値域・単位:** 元と候補framework signature集合（logic、axiom、ontology sort、inference rule、演算意味）のK3距離。[0,1]無次元、Dctxと別。

**算出法:** signatureの対応を明示しK3。二つの同名演算でも意味が異なれば別feature。mere renameは対応規約で同一。

#### `CR-DSEM` · D_sem

$$
D_{\mathrm{sem}}=\operatorname{Avg}_{P_{\mathrm{optional}}}(\mathbf 1[\text{mapped outcomes differ}])
$$

**定義・値域・単位:** 固定nonmandatory意味配置queryに対するbaseline/候補のcanonical outcomeが異なる重み割合、[0,1]無次元。必須保存queryはTL-SEMへ。

**算出法:** 各queryを同一scopeで評価→relation/assignment等のtyped outcome比較→違い1/同じ0/不明[0,1]のK1。真理値を違うlogic間で無写像比較しない。

#### `CR-ACOMP` · A_compat

$$
A_{\mathrm{compat}}=\operatorname{Avg}_{P_{\mathrm{integration}}}(\mathbf 1[\text{bridge check passes}])
$$

**定義・値域・単位:** 固定integration/bridge queryが既存体系と正しく接続する成功重み割合、[0,1]無次元。embedding類似度でない。

**算出法:** 型/意味/roundtrip/実使用を各queryの登録oracleで検査→成功1、失敗0、不明[0,1]→K1 success。hard queryは別gate。

#### `CR-CMATCH` · C_match

$$
C_{\mathrm{match}}=\left(1+\frac{\lvert K-K_0\rvert}{\kappa_M}\right)^{-1}
$$

**定義・値域・単位:** C_match=1/(1+|K−K0|/κ_M)、κ_M>0 bits。[0,1]内で正、無次元。原案v1の未定義計算法をこの有理proxyで補完し、Qとaliasしない。

**算出法:** K/K0をK6で計算し正確有理演算。intervalは|K−K0|の最小/最大を計算後単調写像、K0を跨ぐと上限1。

#### `CR-NKNOWN` · N_known

$$
N_{\mathrm{known}}=\max_{y\in\mathcal C}\{1-d_J(F_z,F_y)\}
$$

**定義・値域・単位:** 固定非空archive内の最大construction-feature類似度max_y(1−K3(Fz,Fy))。[0,1]無次元。人類全体の既知性ではない。

**算出法:** corpus revision・feature universe・canonicalizerをpin→全件の類似度→最大。確認済空archiveはUndefined/EmptyCorpus、未読archiveはUnknown、抽出未対応はUnsupported。既知重複1、確認した全件非重複でのみ小値。

#### `CR-TNC` · T_nc

**定義・値域・単位:** 原式sqrt(D_par D_sem) A_compat、[0,1]無次元。いずれかdistance=0またはA=0で厳密0。

**算出法:** 共有metric nodeを参照しK7のsqrt boundとK4積。完全平方以外をf64だけでExactとしない。入力欠落はNoScalar/Bounded方針に従う。

</details>

<details>

<summary>構造利得 (6)</summary>

#### `CR-COVERAGE` · Δcoverage

$$
\Delta\mathrm{coverage}=\operatorname{Pair}_{P_{\mathrm{coverage}}}(s_z,s_0)
$$

**定義・値域・単位:** 固定coverage-validation query集合で説明/解決可能と確認された割合のpaired改善K2、[-1,1]無次元。

**算出法:** 各queryは実導出・model・execution等のoracleを持つ。baseline/候補のpass 1/fail0/unknown intervalを同じ予算で取得しK2。

#### `CR-COMPRESSION` · Δcompression

$$
\Delta\mathrm{compression}=\frac{K_0-K_z}{\max(K_0,K_z)}
$$

**定義・値域・単位:** (K0−Kz)/max(K0,Kz)∈[-1,1]無次元、Kはbits。必須意味同値/保存が検査済みの場合だけ有効な圧縮利得。

**算出法:** K6で比較対象と必要decoder/dictionaryを同じ規約で符号化→total bit長を算出→比。比較情報をdecoderへ隠すexternal lookupはcost外にできない。保存未確認はPrerequisitePending/NoScalar、失敗はUndefined/PrerequisiteFailedかつcandidate Ineligible。未検証の短縮率を有効gainとしない。

#### `CR-BRIDGE` · Δbridge

$$
\Delta\mathrm{bridge}=\operatorname{Pair}_{P_{\mathrm{bridge}}}(s_z,s_0)
$$

**定義・値域・単位:** 固定cross-domain queryが検証済bridgeで利用可能となる割合のK2改善、[-1,1]無次元。edge数でない。

**算出法:** 各queryにsource/target/goal対応・type/semantic/使用oracleを登録→baseline/候補の成功判定→K2。重複名のedgeは同一primitive。

#### `CR-PREDICT` · Δpredict

$$
\mathrm{Brier}(z)=\frac{\sum_i w_i(p_{z,i}-y_i)^2}{\sum_i w_i},\qquad \Delta\mathrm{predict}=\mathrm{Brier}(z_0)-\mathrm{Brier}(z)
$$

**定義・値域・単位:** Brier(z0)−Brier(z)、Brier=Σwi(pi−yi)^2/W、pi∈[0,1],yi∈{0,1}。[-1,1]無次元。多classは別revision。

**算出法:** 固定prediction-validation dataと観測label、予測時点、model revisionをpin。予測をlabel取得前に凍結。paired squared error差をK2集計。

#### `CR-REDUNDANCY` · Redundancy

$$
\mathrm{Redundancy}=\frac{\sum_j\max(0,n_j-1)}{\sum_j n_j}
$$

**定義・値域・単位:** 固定役割別semantic unit multisetでΣ_class max(0,n_class−1)/Σ_class n_class、[0,1]無次元。必須反復はrole別unitなので別class。

**算出法:** 独立検証された意味同値だけでcanonical classを作りcount。空の有効表現は0と明示し、抽出不足はUnknown。表層同語だけで統合しない。

#### `CR-GSTRUCT` · G_struct

**定義・値域・単位:** F2原下位式a1Δcoverage+a2Δcompression+a3Δbridge+a4Δpredict−a5Redundancy。無次元。範囲[−Σ1..5 ai, Σ1..4 ai]。

**算出法:** 5子nodeと係数をpinしK4で評価。全gainを[0,1]へclipしない。shared evidence dependency groupを表示。

</details>

<details>

<summary>既存構造の再編 (5)</summary>

#### `CR-PRESERVE` · Preserve

$$
\mathrm{Preserve}=\operatorname{Avg}_{P_{\mathrm{struct}}}(\mathbf 1[\text{preserved}])
$$

**定義・値域・単位:** 固定source structural propertyの保存成功重み割合K1、[0,1]無次元。差分でなく絶対値、恒等で通常1。

**算出法:** 各propertyをsource/target対応で検査→成功1/失敗0/不明interval→K1 success。hard subsetを別gate。

#### `CR-REBIND` · Rebind

$$
\mathrm{Rebind}=\operatorname{Avg}_{P_{\mathrm{units}}}(\mathbf 1[\text{changed binding and verified useful map-back/use}])
$$

**定義・値域・単位:** 固定source unitのうち結合先/役割が実際に変わり、有用なmap-backと使用probeが成立した割合、[0,1]無次元。

**算出法:** unitごとの前後bindingを比較→rename除外→再結合のtype/semantic/goal使用を独立oracle確認→成功K1。変化なし0。

#### `CR-REFRAME` · Reframe

$$
\mathrm{Reframe}=\operatorname{Avg}_{P_{\mathrm{perspective}}}(\mathbf 1[\text{baseline fails and reframed mapped candidate passes}])
$$

**定義・値域・単位:** 固定perspective queryのうちbaselineで不成立、明示frame変更＋検証済map-backで候補が説明可能になった重み割合、[0,1]無次元。

**算出法:** 各queryのbaseline failureを確認→frame変更をDeltaで確認→candidate passとroot対応を検査→指標I(base_fail AND mapped_pass)をK1。base unknownはunknown。

#### `CR-BREAK` · BreakCost

$$
\mathrm{BreakCost}=\operatorname{Avg}_{P_{\mathrm{optional\ legacy}}}(\mathbf 1[\text{confirmed break}])
$$

**定義・値域・単位:** 固定optional legacy compatibility/propertyの確認破壊率K1、[0,1]無次元。hard義務や操作権限はcost外のgate。

**算出法:** legacy property suiteを同一条件で実行→違反1/保存0/不明interval→K1。既存の壊れたbaselineも別測定し勝手に免責しない。

#### `CR-RGAIN` · R_gain

**定義・値域・単位:** 原式b1Preserve+b2Rebind+b3Reframe−b4BreakCost。範囲[−b4,b1+b2+b3]無次元。

**算出法:** 4子nodeを同一scopeから取得しK4。Preserveが絶対値であることを保ち、名前gainだからと自動中心化しない。

</details>

<details>

<summary>実用利得 (5)</summary>

#### `CR-EXPLAIN` · Δexplain

$$
\Delta\mathrm{explain}=\operatorname{Pair}_{P_{\mathrm{explain}}}(s_z,s_0)
$$

**定義・値域・単位:** 固定explanation-validation queryの正しい導出/説明成立scoreのpaired gain K2、[-1,1]無次元。

**算出法:** queryごとにcheckable derivation checkerを登録しpass1/fail0を取得。自由proseをLLMが採点する場合はEstimatedかつrubric/model/judge固定、Exact形式領域と同じ保証にしない。

#### `CR-DESIGN` · Δdesign

$$
g_{\uparrow}(x)=\operatorname{clip}_{[0,1]}\left(\frac{x-m}{M-m}\right),\quad g_{\downarrow}(x)=1-g_{\uparrow}(x),\quad \Delta\mathrm{design}=\operatorname{Pair}_{P_{\mathrm{design}}}(s_z,s_0)
$$

**定義・値域・単位:** 固定design acceptance suiteのscore改善K2、[-1,1]無次元。各scoreはbinaryか、事前固定m<MとAcceptedInputRangeを持つquantityをnormalizeし、large-is-betterはclip((x−m)/(M−m),0,1)、small-is-betterは1−clip((x−m)/(M−m),0,1)。m/Mはsaturation scaleでAcceptedInputRangeそのものではない。

**算出法:** AcceptedInputRangeを先に検査。range内だがNormalizationBounds m/M外の値はMapの明示saturationとしてclipし、raw値と上下clip flagを残す。AcceptedInputRange外はInvalidInput。small/large directionはscope固定。unit整合、同じresource budgetと環境でbase/candidateを実測しpaired差。未実測design提案はpredicted。

#### `CR-TRANSFER` · Δtransfer

$$
\Delta\mathrm{transfer}=\operatorname{Pair}_{P_{\mathrm{transfer}}}(s_z,s_0)
$$

**定義・値域・単位:** 事前分離したtransfer-validation familyのscore改善K2、[-1,1]無次元。scoreはbinaryまたはCR-DESIGNの固定量正規化。

**算出法:** source訓練/探索familyとの分割digestを確認→同じ転移budgetでbase/candidateを実行→query別差。最終research holdoutはこの選択入力へ戻さない。

#### `CR-SOLVE` · Δsolve

$$
\Delta\mathrm{solve}=\operatorname{Pair}_{P_{\mathrm{solve}}}(s_z,s_0)
$$

**定義・値域・単位:** 固定root-goal solution suiteで正解/実行可能性/目的品質scoreの改善K2、[-1,1]無次元。各probeのbinaryまたはCR-DESIGN正規化を固定。

**算出法:** 候補の実model/導出/program出力を独立oracleへ渡す→hard feasibilityを先に検査→同じgoal/query/budgetでscore差。形式に合うだけの回答を解決成功にしない。

#### `CR-UGAIN` · U_gain

**定義・値域・単位:** 原式c1Δexplain+c2Δdesign+c3Δtransfer+c4Δsolve。無次元、範囲[−Σci,Σci]。

**算出法:** 4子nodeをK4で評価、各dataset roleとevidence basisを保存。REC-Uと型を共有しない。

</details>

<details>

<summary>記述長・リスク・F1/F2の値域 (6)</summary>

#### `CR-K` · K, K0

$$
K(z)=8\,\operatorname{len}_{\mathrm{bytes}}\!\left(E(\mathrm{Bundle}(z))\right)\ \mathrm{bits},\qquad K_0=K(z_0)>0
$$

**定義・値域・単位:** K(z)=8 len(E(z)) bits、K0=K(z0)>0、整数。実用記述長proxyであり最小program長/計算不能なKolmogorov complexityではない。

**算出法:** K6のtag grammarでsemantic termと必要decoder/dictionaryを同じself-contained Bundle termに符号化。共通runtimeはProfileDigestで固定し双方に等しく扱う。元参照IDで情報を外へ逃さない。

#### `CR-QPARITY` · Q_parity

**定義・値域・単位:** 原式exp(−|K−K0|/σ)、σ>0 bits。有限Kで(0,1]無次元。±同差の単純化/複雑化を等しく扱う。

**算出法:** K6のKと固定σからK7でdmin/dmaxを計算。lower=lowerExp(dmax/σ)。K区間がK0を含めばouter upper=1（Exact1はKがexactly K0の証拠がある場合だけ）。K0を含まずdmin>0ならupper=min(upperExp(dmin/σ),1/(1+dmin/σ))。underflow/roundingの0/1を数学的Exact値にしない。

#### `CR-RCOUNTER` · R_counter

$$
R_{\mathrm{counter}}=\operatorname{Avg}_{P_{\mathrm{adversarial}}}(\mathbf 1[\text{confirmed failure}])
$$

**定義・値域・単位:** 固定非空adversarial probe suiteでの確認failure重み割合K1、[0,1]無次元。反例の発見件数でも一般的反例確率でもない。

**算出法:** 固定ケースを同じsolver/resourceで実行し、反例は別checkerで再評価。failure1/pass0/unobserved[0,1]。試行0/予算0/探索未実施はUnknown。完走suiteでfailure0なら当該有限suite上のみ0。

#### `CR-BMEM` · B_mem

$$
C\in\left[\min_p\sum_{e\in p}\underline c_e,\ \min_p\sum_{e\in p}\overline c_e\right],\qquad B_{\mathrm{mem}}=\frac{C}{C+\kappa_B}
$$

**定義・値域・単位:** 全admissible nonsecurity conversionを列挙した閉じた有限graph上のminimum bridge cost Cを`B_mem=C/(C+κ_B)`で正規化する。κ_B>0は同じtyped cost unit。C∈[0,∞]、B_mem∈[0,1]。Cが区間なら単調mapを両endpointへ適用する。完全な閉グラフでpathなしの場合だけ1とDisconnectedを併記。

**算出法:** 全admissible nonsecurity conversionを列挙した閉じた有限graph、始点終点、nonnegative edge costをpin。costがintervalなら全下限weightでDijkstraを一度、全上限weightで一度実行し、`C=[minPath(lower),minPath(upper)]`をouter boundとする。Graphのedge registryが閉じている場合に限り、path不存在をDisconnectedとする。κ_B変換は単調に両endpointへ適用する。同cost pathはcanonical transition-payload sequenceでtie-breakしopaque IDは使わず、同一content/evidence path群は一つのcost witnessへ統合。pathは独立検査。timeoutはUnknown＋有効bound。早期探索失敗からdisconnectを推測しない。

#### `CR-V1` · EpiRecompScore F1

**定義・値域・単位:** Formula F1そのまま。無次元preference index、範囲[−(λ+η),α+β+γ+δ]。確率でも普遍的創造性でもない。

**算出法:** 6成分・6係数・定義revisionをpinしK4。raw S1とbaseline S1(z0)を別評価してdelta S1を返す。identityでA=C=1、固定archiveがbaseline構造を含んでN=1、Dctx=Dsem=L=0ならγ+δ−η。

#### `CR-V2` · EpiRecompScore F2

**定義・値域・単位:** Formula F2そのまま。G範囲[glo,ghi]、R[rlo,rhi]、U[ulo,uhi]なら下界w2glo+w3rlo+w4ulo−w6−w7−w8、上界w1+w2ghi+w3rhi+w4uhi+w5。無次元。

**算出法:** 全8項と子nodeを同一DAGで評価。raw baselineとdelta S2を別計算。identity Redundancy0、Preserve1、Q1、他0ならS2(z0)=w3b1+w5。Redundancy0>0なら−w2a5Redundancy0も残す。

</details>

<a id="kernels-ja"></a>

### 共通の計算式：K1〜K7

**K1・K2・K3。** 重みは比較前に固定します。ペア差は項ごとに求め、丸めた平均同士を引きません。集合は固定universe内で同じ意味のfeatureを対応づけます。有効と確認済みの空集合対だけを距離0とします。

$$
\begin{aligned}
\operatorname{Avg}_{P}(v)&=\frac{\sum_{i\in P}w_i v_i}{W},\quad W=\sum_{i\in P}w_i>0,\\
\operatorname{Pair}_{P}(s_z,s_0)&=\frac{\sum_i w_i(s_z(i)-s_0(i))}{W},\\
d_J(A,B)&=\frac{\sum_{i\in A\triangle B}v_i}{\sum_{i\in A\cup B}v_i}\quad(A\cup B\ne\varnothing).
\end{aligned}
$$

**K4：有界区間。** $A=[a,b]$, $B=[c,d]$。ゼロを含む除数では未定義です。同じ有効観測ノードの自己差はExact 0ですが、同じ数値区間を持つ別ノードは通常の区間演算のままです。

$$
\begin{aligned}
A+B&=[a+c,b+d],& A-B&=[a-d,b-c],& -A&=[-b,-a],\\
AB&=[\min(ac,ad,bc,bd),\max(ac,ad,bc,bd)],&
A/B&=A[1/d,1/c]\quad (0\notin B),\\
\operatorname{square}(A)&=\begin{cases}[0,\max(a^2,b^2)]&a\le0\le b\\\left[\min(a^2,b^2),\max(a^2,b^2)\right]&\text{otherwise},\end{cases}\\
\operatorname{abs}(A)&=\begin{cases}[0,\max(-a,b)]&a\le0\le b\\\left[\min(|a|,|b|),\max(|a|,|b|)\right]&\text{otherwise}.\end{cases}
\end{aligned}
$$

$$
\{\min_i x_i:x_i\in[\ell_i,u_i]\}\subseteq[\min_i\ell_i,\min_i u_i],\qquad
\{\max_i x_i:x_i\in[\ell_i,u_i]\}\subseteq[\max_i\ell_i,\max_i u_i].
$$

最後の二式は有限で空でない族の外側包含です。空族は未定義です。平方根は非負領域で外向きに評価します。

**K5：古典論理における検査。** まず $\Gamma$ の充足可能性を確かめます。不整合なら古典的な採点を止めます。整合する場合に限り次を使い、両側がSATなら未決定です。solverの報告、再検査したモデル、検査した証明、有限全列挙は別の証拠です。ASP・時相・四値はそれぞれの意味論を使います。

$$
\Gamma\models\varphi\iff\operatorname{UNSAT}(\Gamma\land\neg\varphi),\qquad
\Gamma\models\neg\varphi\iff\operatorname{UNSAT}(\Gamma\land\varphi).
$$

**K6・K7：記述長と数値精度。** K6は自己完結する型付き項・decoder・dictionaryの符号長で、Kolmogorov complexityではありません。K7はGCD正規化された有理数と外向きの区間を使います。$1\le p\le256$、参照精度は64です。下の $q$ は平方根の格子下限を決めます。有理完全平方はExact、それ以外は包含であり、資源超過はResourceExhaustedです。

$$
x=n/d\ge0,\quad q=\operatorname{isqrt}\!\left(\left\lfloor n2^{2p}/d\right\rfloor\right),\quad
\sqrt{x}\in[q/2^p,(q+1)/2^p].
$$

$$
\begin{aligned}
r&=\min\{r\in\mathbb Z_{\ge0}:x/2^r\le1/2\},\quad y=x/2^r,\quad P=p+r+16,\\
T_m(y)&=\sum_{k=0}^{m}(-y)^k/k!,\quad T_{2j+1}(y)\le e^{-y}\le T_{2j}(y),\\
\operatorname{down}_P(v)&=\lfloor v2^P\rfloor/2^P,\quad \operatorname{up}_P(v)=\lceil v2^P\rceil/2^P.
\end{aligned}
$$

Taylor上下界の差を $2^{-P}$ 以下にして外向きに丸めます。$r$ 回、$[l,u]\mapsto[\operatorname{down}_P(l^2),\operatorname{up}_P(u^2)]$ を適用し、最終幅が $2^{-p}$ 以下かを検査します。未達なら元の有理Taylor境界から $P$ を32増やして最大3回再計算します。参照上限は $r\le64$、256項です。不確かな小値をExact 0にしません。

$$
\begin{aligned}
d_{\min}&=\begin{cases}0&k_L\le K_0\le k_U\\\min(|k_L-K_0|,|k_U-K_0|)&\text{otherwise},\end{cases}\\
d_{\max}&=\max(|k_L-K_0|,|k_U-K_0|),\\
e^{-[x,y]}&\subseteq [\operatorname{lowerExp}(y),\operatorname{upperExp}(x)]\quad (0\le x\le y),\\
Q_{\rm parity}&\in [\operatorname{lowerExp}(d_{\max}/\sigma),\operatorname{upperExp}(d_{\min}/\sigma)],\\
\overline Q&=\min(\operatorname{upperExp}(x),1/(1+x))<1\quad(x=d_{\min}/\sigma>0).
\end{aligned}
$$

写像後の値が無効なら自動clipしません。正規化・飽和が登録された設計成分だけが例外です。基準を含む $K$ 区間の上界1はExact 1ではありません。

<a id="policy-ja"></a>

### 候補選択と探索配分（得点とは別）

RootGoalのraw範囲は $m<M$、方向を固定します。有効範囲を外れたExact入力をclipせず拒否し、境界を跨ぐ区間はNeedsEvidenceです。一方、CR-DESIGNの飽和はAcceptedInputRange検査後にのみ適用します。

$$
g_{\max}(x)=(x-m)/(M-m),\qquad g_{\min}(x)=(M-x)/(M-m).
$$

必須条件を先に検査します。上限型の損失gateは $u\le\tau$ でPass、$l>\tau$ でFail。下限型の利得gateは $l\ge\tau$ でPass、$u<\tau$ でFail。他はNeedsEvidenceです。必須保存の $\tau=0$。標準のJointParetoは目標・個別損失・primitive利得・soft costを比較し、F1/F2や複合利得をさらに足しません。

$$
\forall i\in I_{\max}:\ \underline A_i\ge\overline B_i,\qquad
\forall j\in I_{\min}:\ \overline A_j\le\underline B_j.
$$

上の全条件と少なくとも一つの厳密不等号が保守的な支配の条件です。重なる区間から勝者を決めません。strict original scoreで単独の勝者とするのは、他のすべての適格候補に対して得点下界が相手の上界より大きい場合だけです。目標や代表順序が未定義ならNoRecommendation。比較可能なら推奨解一つと重要な代替を残します。

$$
J=aL_{\rm trans}-bS_j:\qquad
[L_{\rm trans}]J=\begin{cases}a+b\lambda&j=1\\a+bw_6&j=2.\end{cases}
$$

これは同じ損失の二重計上を検出するための展開で、標準の選択関数ではありません。

**探索の優先度。** $u$ は適用軸の区間幅／宣言値域幅の最大（固定幅0の軸の寄与は0）。$B>0$ は問い合わせ枠、$\hat c$ は登録した推定slot costです。最初から固定したnovelty policyを使い、不明な場合はcanonical round-robinに戻ります。$\lceil B/8\rceil$ 枠を公平な巡回に取り、残りは優先度下界の降順です。

$$
c=\min(1,\hat c/B),\qquad \pi=\frac{(1+u)(1+n)}{1+c},\qquad
n_{\cup}=d_J(F_x,\bigcup_{y\in\mathcal F}F_y),\quad n_{\rm near}=\min_{y\in\mathcal F}d_J(F_x,F_y).
$$

和集合との距離が参照方針です。最近傍との距離は別方針で、順位は逆転し得ます。以下のK8は幾何を登録した場合だけの別の探索提案です。物理距離、意味の真値、採用や実行の許可ではありません。

$$
\begin{aligned}
\mathbb B_\kappa&=\{x:\kappa\lVert x\rVert_2^2<1\},\qquad \kappa>0,\quad \kappa\lVert x\rVert_2^2\le1-\mu,\quad 0<\mu<1,\\
t&=\frac{\kappa\lVert x-y\rVert_2^2}{(1-\kappa\lVert x\rVert_2^2)(1-\kappa\lVert y\rVert_2^2)},\qquad q=\sqrt{t/(1+t)},\\
d_\kappa(x,y)&=\frac{\operatorname{acosh}(1+2t)}{\sqrt\kappa}
=\frac{2\operatorname{atanh}(q)}{\sqrt\kappa},\\
T_N(q)&=\sum_{k=0}^{N}\frac{q^{2k+1}}{2k+1},\qquad
\operatorname{atanh}(q)\in\left[T_N(q_L),\,T_N(q_U)+\frac{q_U^{2N+3}}{(2N+3)(1-q_U^2)}\right].
\end{aligned}
$$

座標box全体を境界内と確認し、ノルム二乗は成分ごとにK4の専用squareを使います。$t_L,t_U$ の各端点で数値幅 $\le2^{-64}$ を検査します。内側精度80から32ずつ最大256まで、$N=0\ldots256$。$q_U=1$ または未収束なら精度を上げ、上限ではResourceExhausted（有効boundがなければNoScalar）。入力の不確実性による区間幅は数値丸め誤差ではありません。

$$
d_{\rm near}=\min_{y\in\mathcal F}d_\kappa(x,y)\in[d_L,d_U],\qquad
n_{\rm geo}\in[1-\operatorname{upperExp}(d_L/s_{\rm geo}),\,1-\operatorname{lowerExp}(d_U/s_{\rm geo})],\quad s_{\rm geo}>0.
$$

登録embeddingや検証済frontierがなければ、この値を作りません。幾何や蒸留の提案は、離散検証と再測定なしに指標を書き換えません。

<a id="examples-ja"></a>

### 数値例と確認先

重み1と3の二つの検査で損失が0と1ならEpiMapLossは $3/4$ です。二つ目が定義済みだが未観測なら $[0,3/4]$ で、0にも $3/4$ にも確定しません。均等係数の合成恒等例ではF1が $1/6$、F2が $5/32$ で、どちらも自分自身との差は0です。これは算術の試験例であり、開発上の効果を測った数値ではありません。

<details>

<summary>固定契約の全37数値例・境界例（G01〜G37）</summary>

| ID | 入力と期待値（試験用） |
| --- | --- |
| G01 | K1 weights(1,3), losses(0,1) →3/4 |
| G02 | 同weights、losses(0,Unknown[0,1]) →[0,3/4] |
| G03 | K3 {a,b}/{b,c}, 全weight1 →2/3 |
| G04 | paired scores baseline(1,0),candidate(1,1) →1/2 |
| G05 | T: Dpar=1/4,Dsem=1,A=1/2 →1/4 |
| G06 | C_match: K0=80,κ=80,K=160 →1/2 |
| G07 | Q: K0=160,σ=80,K=80と240 →両方exp(-1)≈0.36787944117144232160 |
| G08 | Q: K∈[80,240],K0=160,σ=80 →[exp(-1),1] |
| G09 | uniform synthetic identity, A=C=1, archive includes exact baseline so N=1, Dctx=Dsem=L=0 →S1=1/6 |
| G10 | uniform identity, Preserve=1,Q=1,Rebind=Reframe=BreakCost=Redundancy=0、T=G=U=L=Rcounter=Bmem=0 →S2=5/32 |
| G11 | G10のRedundancy=1/4 →Gstruct=−1/20,S2=3/20 |
| G12 | raw δSはS(z)−実測S(z0)。identity候補のδS=0、rawは非zeroでもよい |
| G13 | loss interval[0.1,0.2],τ=0.2 →Pass；[0.2,0.3] →NeedsEvidence；[0.21,0.3] →Fail |
| G14 | c=3,κ_B=1 →Bmem=3/4；確認no path→1+Disconnected |
| G15 | Δcompression K0=80,K=40かつ保存合格→1/2；保存失敗ならIneligible |
| G16 | Brier baseline p=0,y=1、candidate p=1/2 →Δpredict=3/4 |
| G17 | K4 `[-2,3]×[-4,5]` →`[-12,15]`；除数区間が0を含む →Undefined/DivisionByZero |
| G18 | error e=5, epsilon=2, r=10（同一unit）→ell=3/10；1s→1000ms canonicalize後のidentity error=0 |
| G19 | UNSAT report without checked proof/enumeration →ValidationBasis=SolverReported、not independent validation |
| G20 | Q, K∈[200,240],K0=160,σ=80 →[exp(-1),exp(-1/2)] |
| G21 | Poincare κ=1, x=(0,0), y=(1/2,0) →d=acosh(5/3)=ln3≈1.098612288668109691 |
| G22 | No comparable registered RootGoalProfile →NoRecommendation; retain alternatives |
| G23 | Poincare κ=1 on one diameter: d(0,1/4)=ln(5/3), d(1/4,1/2)=ln(9/5), d(0,1/2)=ln3; the first two sum to the third. Node-ID relabeling leaves distances unchanged |
| G24 | Goal raw range [10,30], v=15 →max score 1/4, min score 3/4; v=31→InvalidInput |
| G25 | Two tied branches A<B by canonical key; relabel IDs leaves dispatch unchanged. Next budget epoch begins after persisted cursor even when frontier revision changed |
| G26 | CR-DESIGN accepted domain [-100,100], normalize [0,10], maximize x=12 →1 with SaturatedHigh; x=101→InvalidInput |
| G27 | B_mem: two paths with cost intervals [1,2]+[1,2] and [3,4] →minimum cost [2,4]; κ_B=2 gives B_mem=[1/2,2/3]. Open graph/no path observation →Unknown, not Disconnected |
| G28 | K4 family min{[0,10],[5,6]} enclosure →[0,6]; max →[5,10]; empty family min/max →Undefined/NoApplicable |
| G29 | Noncrossing Q with dmin/σ=2^-1000: outward exp bound may equal 1, but rational upper `1/(1+2^-1000)<1` retains strict noncrossing |
| G30 | Poincare κ=1, x=(0,0), y=(1/2,1/2), μ=1/4: q=sqrt(1/2); K8 refines internal precision above 64 bits until endpoint numerical slack<=2^-64 or reports ResourceExhausted |
| G31 | `d_near=[0,1]`, `s_geo=1`: `n_geo` encloses `[0,1−e^-1]` and remains ordered (`lower<=upper`) |
| G32 | Poincare κ=1, x=(1/2,0), y=(0,1/2): exact `t=8/9`, `q²=8/17`; K8 uses internal precision above 64 bits or returns a bounded ResourceExhausted result, never false convergence |
| G33 | Poincare κ=1, x∈[0,1/10], y=1/2: sound distance enclosure includes `[ln(27/11),ln3]`, width `ln(11/9)`; the large input interval is not a numeric-precision failure |
| G34 | `square([-1/10,1/10])` returns `[0,1/100]`; no negative norm-square lower bound is introduced |
| G35 | Frontier `{{a},{b}}`: for proposal `{a}` and `{a,b}`, union-distance values are `1/2,0`, nearest-verified-candidate distances are `0,1/2`; each is bound to a distinct `SchedulerNoveltyPolicyId` |
| G36 | New feature outside the fixed K3 universe remains `InvalidInput/UniverseMismatch`; a new ComparisonScope revision is required to compare it |
| G37 | Missing/unverified EmbeddingPolicy remains `Unsupported`; search proposal generation cannot create authority or exact semantic novelty |

</details>

[README](../README.md) · [固定測定契約](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) · [Beta 6.9保存記録](epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md)

---

## English

This page explains how Epistesys represents what is lost in a transformation and how it evaluates a candidate reorganization. The source of the equations is **finite-formal-v1.4**, the [fixed measurement contract](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) and [design revision](../validation/tlcx/design-revision-chain-2026-10-29.json) published in the Beta 6.9 checkpoint.

**Reading order:** [Mapping preservation loss](#map-en) → [recomposition F1, F2 and their subformulas](#score-en) → [all 40 components](#catalog-en). For implementation or numerical checks, see [shared calculations](#kernels-en), [selection and search](#policy-en), and [numerical examples](#examples-en).

`EpiMapLoss` (mapping preservation loss) and `EpiRecompScore` (structural recomposition score) are the current public names used here. Formula IDs, mathematical notation, and existing Rust/JSON compatibility names are unchanged. These names do not introduce new Rust types or wire fields.

**Notation:** $z$ is the candidate and $z_0$ the baseline in the same comparison scope. $P$ is a fixed set of probes; $w_i>0$ is a probe weight. The equations require compatible scope, baseline, applicability, units, verification method and budget. `Exact` describes arithmetic on the supplied inputs.

<a id="map-en"></a>

### EpiMapLoss: what was lost in a mapping

For a mapping from input $X$ to representation $Y_i$, test properties that should survive. The four axes are reconstruction, semantics, time and space. The original concept specified the mappings and axes but not a scalar reduction; the following aggregation is the current operational convention.

$$
X\xrightarrow{F_i}Y_i,\qquad \mathcal A\subseteq\{\mathrm{recon},\mathrm{sem},\mathrm{time},\mathrm{space}\}.
$$

$$
\begin{aligned}
W_a &= \sum_{q\in P_a}w_q,\qquad
L_a=\left[\frac{\sum_q w_q\underline\ell_q}{W_a},\frac{\sum_q w_q\overline\ell_q}{W_a}\right],\\
\operatorname{EpiMapLoss}_{\mathcal A,\rho}(z)
&=L_{\mathrm{trans}}(z)=\frac{\sum_{a\in\mathcal A}\rho_a L_a(z)}{\sum_{a\in\mathcal A}\rho_a},\qquad \rho_a>0 .
\end{aligned}
$$

A confirmed preserved relation has probe loss 0; a confirmed violation has loss 1. Use $[0,1]$ only when the quantity and range are defined but observation is missing. Undefined, incomparable, or unsupported inputs are not zero. Do not change the mask or denominator per candidate or average away hard violations.

$$
e_q=\begin{cases}
\lvert v_t-F_q(v_s)\rvert&\text{registered scalar},\\
\lVert v_t-F_q(v_s)\rVert_{\text{registered norm}}&\text{registered vector norm},
\end{cases}
\qquad
\ell_q=\min\!\left(1,\frac{\max(0,e_q-\epsilon_q)}{r_q}\right).
$$

Tolerance $\epsilon_q\ge0$ and saturation scale $r_q>0$ have the same units as the error. Spatial probes use a registered comparator. For time, distinguish instants from durations. A nonempty interval requires $s<e$ and contains instants $s\le t<e$; before means $e_A<s_B$ and meets means $e_A=s_B$.

$$
F_{\mathrm{abs}}(t)=at+b,\qquad F_{\mathrm{dur}}(\Delta t)=a\,\Delta t.
$$

One [current Rust profile](../scripts/lc631/crates/lc631-runtime/src/checked_tl_reduction.rs) is a finite-MODEL comparison with temporal and spatial axes fixed at weights 1:3. Those weights are not defaults for all tasks. End-to-end loss rechecks the original properties at the final representation; it is not defined as a sum over edges.

$$
L_{\mathrm{tuple}}=\frac{L_{\mathrm{time}}+3L_{\mathrm{space}}}{4},\qquad L_{\mathrm{end}}=L(X,Y_{\mathrm{final}};\,P_{\mathrm{root}}).
$$

Each transformation has four records $(D,\Delta,U,G)$: confirmed preservation defects, permitted premise changes, unresolved items, and baseline-relative gain records. These are not four numbers to add. Distinguish an intentional premise change from a defect, and predicted from observed gains.

<a id="score-en"></a>

### EpiRecompScore: F1, F2, and all subformulas

**F1** (fixed ID `OriginalFormula/F1-v1`). This profile evaluates contextual and semantic distance, compatibility, and complexity balance, subtracting mapping preservation loss and knownness. $\operatorname{EpiRecompScore}_{F1}(z)=S_1(z)$.

$$
\begin{aligned}
S_1(z)&=\alpha D_{\mathrm{ctx}}(z)+\beta D_{\mathrm{sem}}(z) \\
&\quad -\lambda L_{\mathrm{trans}}(z)+\gamma A_{\mathrm{compat}}(z) \\
&\quad +\delta C_{\mathrm{match}}(z)-\eta N_{\mathrm{known}}(z).
\end{aligned}
$$

**F2** (fixed ID `OriginalFormula/F2-v1`). This profile evaluates novelty with compatibility, structural gain, reorganization, utility, and complexity parity, subtracting mapping preservation loss, counterexample risk, and bridging cost. $\operatorname{EpiRecompScore}_{F2}(z)=S_2(z)$.

$$
\begin{aligned}
S_2(z)&=w_1T_{\mathrm{nc}}(z)+w_2G_{\mathrm{struct}}(z) \\
&\quad +w_3R_{\mathrm{gain}}(z)+w_4U_{\mathrm{gain}}(z) \\
&\quad +w_5Q_{\mathrm{parity}}(z)-w_6L_{\mathrm{trans}}(z) \\
&\quad -w_7R_{\mathrm{counter}}(z)-w_8B_{\mathrm{mem}}(z).
\end{aligned}
$$

**Novelty–compatibility tension**

$$
T_{\mathrm{nc}}(z)=\sqrt{D_{\mathrm{par}}(z)D_{\mathrm{sem}}(z)}
\,A_{\mathrm{compat}}(z).
$$

**Structural gain**

$$
G_{\mathrm{struct}}=
a_1\Delta coverage+a_2\Delta compression+a_3\Delta bridge
+a_4\Delta predict-a_5Redundancy.
$$

**Preservation and reorganization**

$$
R_{\mathrm{gain}}=
b_1Preserve+b_2Rebind+b_3Reframe-b_4BreakCost.
$$

**Utility gain**

$$
U_{\mathrm{gain}}=
c_1\Delta explain+c_2\Delta design+c_3\Delta transfer+c_4\Delta solve.
$$

**Complexity parity**

$$
Q_{\mathrm{parity}}(z)=
\exp\!\left(-\frac{|K(z)-K_0|}{\sigma}\right).
$$

$D_{\rm ctx}$ in F1 and $D_{\rm par}$ in F2 are distinct; so are $C_{\rm match}$ and $Q_{\rm parity}$. $D_{\rm sem}$ measures differences in nonmandatory semantic configuration, not the required-preservation loss $L_{\rm sem}$. Record $U$ is not $U_{\rm gain}$, and record $G$ is not $G_{\rm struct}$.

$$
\Delta S_j=S_j(z)-S_j(z_0),\qquad j\in\{1,2\}.
$$

Report the raw score separately from its change relative to baseline. Even an identity candidate need not have raw score zero. There are 27 original coefficients (F1: 6; F2: 8; structural: 5; reorganization: 4; utility: 4). The convention uses finite nonnegative rationals; the original does not require unit-sum coefficients. $\sigma,\kappa_M$ are positive in bits; $\kappa_B$ is positive in bridge-cost units. Uniform coefficients are test preferences, not calibrated production weights.

These are outer ranges for nonnegative coefficients. The uniform values belong only to `reference-uniform-test-v1`; the fixture specifies the bridge-cost scale $\kappa_B$.

$$
\begin{aligned}
S_1&\in[-(\lambda+\eta),\,\alpha+\beta+\gamma+\delta],\\
G_{\rm struct}&\in[-\sum_{i=1}^{5}a_i,\,\sum_{i=1}^{4}a_i]=[g_l,g_h],\\
R_{\rm gain}&\in[-b_4,\,b_1+b_2+b_3]=[r_l,r_h],\qquad
U_{\rm gain}\in[-\sum_{i=1}^{4}c_i,\,\sum_{i=1}^{4}c_i]=[u_l,u_h],\\
S_2&\in[w_2g_l+w_3r_l+w_4u_l-w_6-w_7-w_8,\;w_1+w_2g_h+w_3r_h+w_4u_h+w_5].
\end{aligned}
$$

$$
(\alpha,\beta,\lambda,\gamma,\delta,\eta)=(1/6,\ldots,1/6),\qquad
w_i=1/8,\quad a_i=1/5,\quad b_i=c_i=1/4,\quad \sigma=\kappa_M=K_0.
$$

The [Rust formula evaluator](../scripts/lc631/crates/lc631-analysis/src/creativity_measurement/formula/evaluate.rs) does not fill missing components with zero. A zero coefficient does not rescue an undefined input: when strict F1/F2 cannot be evaluated, the result is `NoScalar`. The [runtime preflight](../scripts/lc631/crates/lc631-runtime/src/original_score_pending.rs) retains Pending rather than producing an evaluated score when inputs are unavailable. Arithmetic implementation, component production, end-to-end closure, and independent effects are separate.

<a id="catalog-en"></a>

### All 40 components: definitions, ranges, and calculations

This is the component catalogue for the same revision. $\operatorname{Avg}$ denotes the fixed weighted mean, $\operatorname{Pair}$ a paired difference against baseline, and $d_J$ weighted set distance. Indicator values are 0 or 1 only when checked; missing observations do not become zero. Record sets are not assigned invented scalar formulas. See the [normative contract](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) for applicability, independent checks, and effects on selection.

<details>

<summary>Mapping preservation loss and auxiliary quantities (7)</summary>

#### `TL-RECON` · reconstruction loss

**Definition, range, and units:** Fraction of protected source features unreconstructable by an allowed inverse H(Y). Apply K1 to fixed P_recon. L_recon is dimensionless in [0,1], distinct from byte similarity.

**Calculation:** Give only target semantic payload to an isolated decoder; reconstruct; compare using fixed feature oracles; map failure/success/unobserved to 1/0/[0,1]; apply K1. Pin decoder, dictionary, model and side information, also accounting for them in compression cost.

#### `TL-SEM` · semantic preservation loss

**Definition, range, and units:** Weighted fraction violating required semantic-preservation probes, each explicitly declaring equivalence, soundness or completeness. K1 gives a dimensionless [0,1] loss, not creative D_sem.

**Calculation:** Lower source/target queries using a fixed mapping and check each context's consistency first. For a satisfiable Γ, mark φ Supported only when Γ∧¬φ is UNSAT; Refuted only when Γ∧φ is UNSAT; both SAT means Undetermined. Equivalence requires both directional entailment queries Supported; soundness requires every target-claimed φ_t's reverse map be Supported by the source; completeness requires every Required source claim φ_s map to a target-supported claim. An UNSAT context is ContextInconsistent. Conflict/Unknown/timeout/Unsupported does not fix a loss; retain its typed reason. Score 1 only for independently confirmed violation and 0 only for complete pass.

#### `TL-TIME` · temporal preservation loss

**Definition, range, and units:** Violations of fixed temporal-order, duration or absolute-time probes. Binary relations score 0/1. Numeric probes compute `e_q=|v_target−F_q(v_source)|` in a common canonical unit, then `ell=min(1,max(0,e_q−epsilon_q)/r_q)`. r_q>0 and allowed discrepancy epsilon_q>=0 share e_q's unit; K1 aggregates to dimensionless [0,1].

**Calculation:** Validate ClockId, TimeScale, Origin, Unit, Precision and ReferenceId; first convert exactly to common canonical units. F maps canonical source time to canonical target time. Absolute instants use `F_abs(t)=a*t+b`; elapsed durations use `F_dur(Δt)=a*Δt`, without b. Intervals are nonempty `[start,end)` with start<end; reversed/empty intervals are InvalidInput and instantaneous events are a separate type; before means endA<startB and meets means equality. Order, duration and absolute timestamps are separate probes. Apply epsilon/r_q to error bounds with outward rounding.

#### `TL-SPACE` · spatial preservation loss

**Definition, range, and units:** Violations of fixed spatial topology, containment, direction or metric probes. Relations score 0/1. Numeric probes use a ComparatorId-fixed `e_q=ComparatorId(F(source),target)` and `ell=min(1,max(0,e_q−epsilon_q)/r_q)`. epsilon_q>=0 and r_q>0 share e_q's unit; K1 yields dimensionless [0,1].

**Calculation:** Validate FrameId, dimension, axes, origin, unit, handedness, object correspondence and TransformClass. After allowed mapping, use registered ComparatorIds for distances/angles/containment errors. Pin vector NormId, angular wrap rule, epsilon and r_q to the policy revision; missing comparators are Unsupported. Propagate transform/measurement intervals through to loss.

#### `TL-TOTAL` · EpiMapLoss / L_trans

**Definition, range, and units:** L_trans=Σ_(a∈A)ρ_a L_a/Σρ_a with positive weights and pre-fixed applicable axes A. Dimensionless [0,1]. The original left aggregation unspecified; this convention completes it.

**Calculation:** Fix mask/weights from the root context; validate axis results; aggregate intervals by rational weighted sums. W=0 is Undefined. Exclude N/A axes only when constructing the scope, never per candidate.

#### `TL-PATH` · end-to-end preservation

**Definition, range, and units:** L_end in [0,1] directly rechecks root-source protected probes at the final target. It is not a sum of edge losses. D-ledger defects use unique (root obligation, property, revision) keys.

**Calculation:** Store per-edge mappings/witnesses; check composition; remeasure final-target TL axes; coalesce the same root defect rather than repeatedly charging it.

#### `TL-MODAL` · modal_likeness

**Definition, range, and units:** Weighted fraction of fixed required capabilities confirmed supported; success-form K1, dimensionless [0,1]. This auxiliary suitability is not a truth probability.

**Calculation:** Require both registry declarations and revision-bound execution probes. Declaration alone is unobserved. Average success 1, confirmed unsupported 0 and unknown [0,1] over a fixed denominator.

</details>

<details>

<summary>Four records per transformation (4)</summary>

#### `REC-D` · four-record Defect

**Definition, range, and units:** D is a finite defect-event set, not a scalar. Keys identify root obligation, property and source/target revisions; each event links TL axes and evidence. Event count is not TL.

**Calculation:** Create only from confirmed preservation violations; deduplicate by root key; link spans, transforms and observations. Unobserved cases belong in REC-U, not confirmed D.

#### `REC-DELTA` · four-record permitted change

**Definition, range, and units:** Delta is a typed edit set carrying original/candidate contexts, targets, before/after, reason, permission basis and map-back. Its count is not distance or gain.

**Calculation:** Extract edits to mutable premises/frameworks; reject intersection with hard duties; record logic-specific meaning; feed Dctx/Dpar/Dsem. Keep the original context immutable.

#### `REC-U` · four-record uncertainty

**Definition, range, and units:** U is a set of unobserved, ambiguous, incomparable, unsupported or unverified gaps, not probability or U_gain. Report counts/scopes without collapsing them to confidence.

**Calculation:** Record unresolved metric/probe states and reasons by unique keys, retaining required evidence and next actions. Never collapse all states to false.

#### `REC-G` · four-record gain

**Definition, range, and units:** G is an event set for validated preservation, reorganization or capability change relative to a fixed baseline. Events reference metrics, primitive features and paired observations; it is not G_struct itself.

**Calculation:** Tag observed and predicted gains separately; detect duplicates through shared primitive IDs; aggregate into creative components. Removing hard conditions is not gain.

</details>

<details>

<summary>Context, novelty, and compatibility (7)</summary>

#### `CR-DCTX` · D_ctx

$$
D_{\mathrm{ctx}}=d_J(P_0^{\mathrm{mutable}},P_z^{\mathrm{mutable}})
$$

**Definition, range, and units:** K3 distance between mutable premise/fact feature sets of original and reconstructed contexts; dimensionless [0,1]. Axiom/inference-rule signatures belong separately to D_par.

**Calculation:** Canonicalize both sets in the pinned universe; compute weighted symmetric difference over union. Changes to immutable features fail a hard gate first.

#### `CR-DPAR` · D_par

$$
D_{\mathrm{par}}=d_J(\Sigma_0,\Sigma_z)
$$

**Definition, range, and units:** K3 distance between framework signatures: logic, axioms, ontology sorts, inference rules and operator meanings. Dimensionless [0,1], separate from Dctx.

**Calculation:** Declare signature correspondence and apply K3. Identically named operators with different meanings are distinct features; mere renaming is invariant.

#### `CR-DSEM` · D_sem

$$
D_{\mathrm{sem}}=\operatorname{Avg}_{P_{\mathrm{optional}}}(\mathbf 1[\text{mapped outcomes differ}])
$$

**Definition, range, and units:** Weighted fraction of fixed nonmandatory meaning-configuration queries whose canonical outcomes differ between baseline and candidate, dimensionless [0,1]. Required preservation belongs to TL-SEM.

**Calculation:** Evaluate queries in one scope; compare typed relations/assignments; apply K1 to different 1, same 0 and unknown [0,1]. Cross-logic outcomes require mappings.

#### `CR-ACOMP` · A_compat

$$
A_{\mathrm{compat}}=\operatorname{Avg}_{P_{\mathrm{integration}}}(\mathbf 1[\text{bridge check passes}])
$$

**Definition, range, and units:** Weighted success fraction of fixed integration/bridge queries connecting correctly to the existing system; dimensionless [0,1], not embedding similarity.

**Calculation:** Check type, meaning, roundtrip and actual use with registered per-query oracles; apply success-K1 to pass 1, fail 0 and unknown [0,1]. Hard queries gate separately.

#### `CR-CMATCH` · C_match

$$
C_{\mathrm{match}}=\left(1+\frac{\lvert K-K_0\rvert}{\kappa_M}\right)^{-1}
$$

**Definition, range, and units:** C_match=1/(1+|K−K0|/κ_M), with positive κ_M in bits. Positive dimensionless value up to 1. This rational proxy completes an unspecified v1 algorithm; it is not Q.

**Calculation:** Compute K/K0 by K6 and exact rational arithmetic. For intervals find minimum/maximum absolute differences before the monotone map; crossing K0 gives upper bound 1.

#### `CR-NKNOWN` · N_known

$$
N_{\mathrm{known}}=\max_{y\in\mathcal C}\{1-d_J(F_z,F_y)\}
$$

**Definition, range, and units:** Maximum construction-feature similarity max_y(1−K3(Fz,Fy)) in a fixed nonempty archive; dimensionless [0,1], not universal human knownness.

**Calculation:** Pin corpus, universe and canonicalizer; compute all similarities and take the maximum. Confirmed empty archives are Undefined/EmptyCorpus; unread archives are Unknown and unsupported extraction is Unsupported. Small values require actual archive comparison.

#### `CR-TNC` · T_nc

**Definition, range, and units:** Original sqrt(D_par D_sem) A_compat, dimensionless [0,1]. Either distance zero or A zero gives exact zero.

**Calculation:** Reference shared metric nodes; use K7 square-root bounds and K4 multiplication. Non-square f64 results are not Exact. Missing inputs follow NoScalar/bounding rules.

</details>

<details>

<summary>Structural gain (6)</summary>

#### `CR-COVERAGE` · Δcoverage

$$
\Delta\mathrm{coverage}=\operatorname{Pair}_{P_{\mathrm{coverage}}}(s_z,s_0)
$$

**Definition, range, and units:** Paired K2 improvement in the confirmed fraction of fixed coverage-validation queries explained/solved, dimensionless [-1,1].

**Calculation:** Attach derivation/model/execution oracles per query; obtain baseline/candidate pass 1, fail 0 or unknown intervals under equal budgets; apply K2.

#### `CR-COMPRESSION` · Δcompression

$$
\Delta\mathrm{compression}=\frac{K_0-K_z}{\max(K_0,K_z)}
$$

**Definition, range, and units:** (K0−Kz)/max(K0,Kz), dimensionless [-1,1], with K in bits. Valid compression gain requires checked required equivalence/preservation.

**Calculation:** Encode artifacts plus necessary decoders/dictionaries under the same K6 rules; compare total lengths. External lookup hiding information is not free. Unchecked preservation is PrerequisitePending/NoScalar; failed preservation is Undefined/PrerequisiteFailed and candidate Ineligible. Unverified shortening is not valid gain.

#### `CR-BRIDGE` · Δbridge

$$
\Delta\mathrm{bridge}=\operatorname{Pair}_{P_{\mathrm{bridge}}}(s_z,s_0)
$$

**Definition, range, and units:** K2 improvement in fixed cross-domain queries usable through checked bridges; dimensionless [-1,1], not edge count.

**Calculation:** Register source/target/root-goal correspondence plus type/semantic/use oracles; compare baseline/candidate query success. Renamed duplicate edges share one primitive.

#### `CR-PREDICT` · Δpredict

$$
\mathrm{Brier}(z)=\frac{\sum_i w_i(p_{z,i}-y_i)^2}{\sum_i w_i},\qquad \Delta\mathrm{predict}=\mathrm{Brier}(z_0)-\mathrm{Brier}(z)
$$

**Definition, range, and units:** Brier(z0)−Brier(z), where Brier=Σwi(pi−yi)^2/W, pi∈[0,1], yi∈{0,1}. Dimensionless [-1,1]; multiclass requires another revision.

**Calculation:** Pin prediction-validation data, observed labels, timestamps and model revisions. Freeze predictions before labels are acquired; aggregate paired squared-error differences by K2.

#### `CR-REDUNDANCY` · Redundancy

$$
\mathrm{Redundancy}=\frac{\sum_j\max(0,n_j-1)}{\sum_j n_j}
$$

**Definition, range, and units:** Σ_class max(0,n_class−1)/Σ_class n_class over role-specific semantic-unit multisets; dimensionless [0,1]. Required repetitions occupy distinct role classes.

**Calculation:** Form classes only with checked equivalence, then count. A confirmed empty valid representation gives 0; incomplete extraction is Unknown. Surface word identity is insufficient.

#### `CR-GSTRUCT` · G_struct

**Definition, range, and units:** Original subformula a1Δcoverage+a2Δcompression+a3Δbridge+a4Δpredict−a5Redundancy. Dimensionless range [−Σ1..5 ai, Σ1..4 ai].

**Calculation:** Pin five child nodes and weights; evaluate by K4 without clipping signed gains. Expose shared evidence-dependency groups.

</details>

<details>

<summary>Reorganization (5)</summary>

#### `CR-PRESERVE` · Preserve

$$
\mathrm{Preserve}=\operatorname{Avg}_{P_{\mathrm{struct}}}(\mathbf 1[\text{preserved}])
$$

**Definition, range, and units:** K1 weighted fraction of fixed source structural properties preserved, dimensionless [0,1]. Absolute, not a delta; identity usually gives 1.

**Calculation:** Check source/target correspondence per property; aggregate pass/fail/unknown by success-K1. Gate the hard subset separately.

#### `CR-REBIND` · Rebind

$$
\mathrm{Rebind}=\operatorname{Avg}_{P_{\mathrm{units}}}(\mathbf 1[\text{changed binding and verified useful map-back/use}])
$$

**Definition, range, and units:** Fraction of fixed source units genuinely rebound to changed roles/targets with useful map-back and passing use probes; dimensionless [0,1].

**Calculation:** Compare bindings; exclude renaming; independently check type, semantics and root-goal use; apply success-K1. No change scores 0.

#### `CR-REFRAME` · Reframe

$$
\mathrm{Reframe}=\operatorname{Avg}_{P_{\mathrm{perspective}}}(\mathbf 1[\text{baseline fails and reframed mapped candidate passes}])
$$

**Definition, range, and units:** Weighted fraction of fixed perspective queries failing at baseline but explained after explicit reframing and checked map-back; dimensionless [0,1].

**Calculation:** Confirm baseline failure, Delta frame change, candidate pass and root correspondence; apply K1 to I(base_fail AND mapped_pass). Unknown baseline stays unknown.

#### `CR-BREAK` · BreakCost

$$
\mathrm{BreakCost}=\operatorname{Avg}_{P_{\mathrm{optional\ legacy}}}(\mathbf 1[\text{confirmed break}])
$$

**Definition, range, and units:** K1 failure fraction of fixed optional legacy compatibility/properties, dimensionless [0,1]. Hard duties and authority remain separate gates.

**Calculation:** Run a fixed legacy suite under equal conditions; aggregate violation 1, preservation 0 and unknown intervals. Measure already-broken baseline conditions separately rather than silently exempting them.

#### `CR-RGAIN` · R_gain

**Definition, range, and units:** Original b1Preserve+b2Rebind+b3Reframe−b4BreakCost, dimensionless range [−b4,b1+b2+b3].

**Calculation:** Read four same-scope child nodes and apply K4. Preserve is absolute; the word gain does not authorize automatic recentering.

</details>

<details>

<summary>Utility gain (5)</summary>

#### `CR-EXPLAIN` · Δexplain

$$
\Delta\mathrm{explain}=\operatorname{Pair}_{P_{\mathrm{explain}}}(s_z,s_0)
$$

**Definition, range, and units:** Paired K2 gain in correct derivation/explanation scores on fixed explanation-validation queries, dimensionless [-1,1].

**Calculation:** Register a checkable derivation oracle per query for pass 1/fail 0. LLM prose judgments are Estimated with pinned rubric/model/judge, never Exact formal guarantees.

#### `CR-DESIGN` · Δdesign

$$
g_{\uparrow}(x)=\operatorname{clip}_{[0,1]}\left(\frac{x-m}{M-m}\right),\quad g_{\downarrow}(x)=1-g_{\uparrow}(x),\quad \Delta\mathrm{design}=\operatorname{Pair}_{P_{\mathrm{design}}}(s_z,s_0)
$$

**Definition, range, and units:** K2 improvement on a fixed design-acceptance suite, dimensionless [-1,1]. Per-probe scores are binary or a quantity with fixed m<M normalization bounds and a separate AcceptedInputRange: large-is-better uses clip((x−m)/(M−m),0,1); small-is-better uses 1−clip((x−m)/(M−m),0,1). m/M are saturation bounds, not the accepted input domain.

**Calculation:** Validate AcceptedInputRange first. Values within that domain but outside normalization bounds m/M are explicitly saturated and retain raw value plus lower/upper clip flag; values outside AcceptedInputRange are InvalidInput. Pin direction in scope. Check units, measure both candidates with equal resources/environment and take paired differences. Unexecuted proposals are predicted.

#### `CR-TRANSFER` · Δtransfer

$$
\Delta\mathrm{transfer}=\operatorname{Pair}_{P_{\mathrm{transfer}}}(s_z,s_0)
$$

**Definition, range, and units:** K2 gain on a pre-separated transfer-validation family, dimensionless [-1,1]. Scores use binary checks or CR-DESIGN's fixed normalization.

**Calculation:** Check split digests against training/search families; execute baseline/candidate under equal transfer budgets; compute paired differences. Never feed the final research holdout into selection.

#### `CR-SOLVE` · Δsolve

$$
\Delta\mathrm{solve}=\operatorname{Pair}_{P_{\mathrm{solve}}}(s_z,s_0)
$$

**Definition, range, and units:** K2 improvement in correctness, feasibility or objective-quality scores on fixed root-goal solution queries, dimensionless [-1,1]. Pin binary or CR-DESIGN normalization per probe.

**Calculation:** Give actual models/derivations/program outputs to independent oracles; check hard feasibility first; compare the same goal, queries and budgets. Format compliance alone is not solving.

#### `CR-UGAIN` · U_gain

**Definition, range, and units:** Original c1Δexplain+c2Δdesign+c3Δtransfer+c4Δsolve, dimensionless in [−Σci,Σci].

**Calculation:** Evaluate four child nodes by K4, retaining dataset roles and evidence bases. Do not share its type with REC-U.

</details>

<details>

<summary>Description length, risk, and bounds of F1/F2 (6)</summary>

#### `CR-K` · K, K0

$$
K(z)=8\,\operatorname{len}_{\mathrm{bytes}}\!\left(E(\mathrm{Bundle}(z))\right)\ \mathrm{bits},\qquad K_0=K(z_0)>0
$$

**Definition, range, and units:** K(z)=8 len(E(z)) bits with positive integer K0=K(z0). This practical description-length proxy is not minimal program length or Kolmogorov complexity.

**Calculation:** Encode semantic terms and required decoders/dictionaries as one self-contained Bundle term under K6. Pin shared runtime in ProfileDigest and treat both sides equally. Reference IDs cannot hide information externally.

#### `CR-QPARITY` · Q_parity

**Definition, range, and units:** Original exp(−|K−K0|/σ), positive σ in bits. Dimensionless (0,1] for finite K; equal simplification/complexification changes are symmetric.

**Calculation:** Use shared K6 lengths, pinned σ and K7 to compute dmin/dmax. Lower=lowerExp(dmax/σ). If the K interval contains K0, an outer upper of 1 is allowed (Exact 1 requires evidence that K is exactly K0). If it excludes K0 and dmin>0, use upper=min(upperExp(dmin/σ),1/(1+dmin/σ)). Never label underflow/rounding to 0/1 as a mathematical Exact value.

#### `CR-RCOUNTER` · R_counter

$$
R_{\mathrm{counter}}=\operatorname{Avg}_{P_{\mathrm{adversarial}}}(\mathbf 1[\text{confirmed failure}])
$$

**Definition, range, and units:** K1 confirmed-failure fraction on a fixed nonempty adversarial suite; dimensionless [0,1], neither number of discovered counterexamples nor universal risk probability.

**Calculation:** Run fixed cases under equal resources and independently recheck countermodels. Aggregate fail 1, pass 0 and unobserved [0,1]. Zero trials/budget means Unknown. Completed zero-failure suites give zero only for that finite suite.

#### `CR-BMEM` · B_mem

$$
C\in\left[\min_p\sum_{e\in p}\underline c_e,\ \min_p\sum_{e\in p}\overline c_e\right],\qquad B_{\mathrm{mem}}=\frac{C}{C+\kappa_B}
$$

**Definition, range, and units:** Normalize minimum bridge cost C in a closed finite registry of admissible nonsecurity conversions as C/(C+κ_B), with positive same-unit κ_B, yielding [0,1]. Map cost intervals through this monotone function endpointwise. Only proven path absence in the closed graph gives 1 plus Disconnected.

**Calculation:** Enumerate and pin a closed finite admissible nonsecurity conversion graph, endpoints and nonnegative edge costs. For interval costs, run Dijkstra once on all lower edge weights and once on all upper edge weights to obtain the outer bound `C=[minPath(lower),minPath(upper)]`. Prove disconnection only when the edge registry is closed. Apply the monotone κ_B map to both endpoints. Break equal-cost path ties by canonical transition-payload sequence, never opaque IDs; merge identical canonical content/evidence paths into one cost witness; independently check witnesses. Timeouts retain Unknown and valid bounds. Early search failure never proves disconnection.

#### `CR-V1` · EpiRecompScore F1

**Definition, range, and units:** Formula F1 unchanged: a dimensionless preference index in [−(λ+η),α+β+γ+δ], neither probability nor universal creativity.

**Calculation:** Pin six components, six coefficients and revisions; evaluate by K4. Evaluate baseline separately and return delta S1. Identity with A=C=1, a fixed archive containing the baseline construction (N=1), and Dctx=Dsem=L=0 gives γ+δ−η.

#### `CR-V2` · EpiRecompScore F2

**Definition, range, and units:** Formula F2 unchanged. With G in [glo,ghi], R in [rlo,rhi], U in [ulo,uhi], lower bound is w2glo+w3rlo+w4ulo−w6−w7−w8 and upper bound w1+w2ghi+w3rhi+w4uhi+w5. Dimensionless.

**Calculation:** Evaluate eight terms and their children in one DAG. Compute raw baseline and delta S2 separately. Identity with zero redundancy, Preserve=1, Q=1 and others zero gives w3b1+w5; retain −w2a5Redundancy0 for nonzero baseline redundancy.

</details>

<a id="kernels-en"></a>

### Shared calculations: K1–K7

**K1, K2, K3.** Fix weights before comparing. Subtract paired item scores before aggregating, not rounded means. Identify features in the pinned universe. Only a confirmed valid empty/empty pair has set distance zero by convention.

$$
\begin{aligned}
\operatorname{Avg}_{P}(v)&=\frac{\sum_{i\in P}w_i v_i}{W},\quad W=\sum_{i\in P}w_i>0,\\
\operatorname{Pair}_{P}(s_z,s_0)&=\frac{\sum_i w_i(s_z(i)-s_0(i))}{W},\\
d_J(A,B)&=\frac{\sum_{i\in A\triangle B}v_i}{\sum_{i\in A\cup B}v_i}\quad(A\cup B\ne\varnothing).
\end{aligned}
$$

**K4: bounded intervals.** Let $A=[a,b]$, $B=[c,d]$. Division is undefined for a divisor containing zero. A valid observation node minus itself is exactly zero; distinct nodes with identical bounds still use interval subtraction.

$$
\begin{aligned}
A+B&=[a+c,b+d],& A-B&=[a-d,b-c],& -A&=[-b,-a],\\
AB&=[\min(ac,ad,bc,bd),\max(ac,ad,bc,bd)],&
A/B&=A[1/d,1/c]\quad (0\notin B),\\
\operatorname{square}(A)&=\begin{cases}[0,\max(a^2,b^2)]&a\le0\le b\\\left[\min(a^2,b^2),\max(a^2,b^2)\right]&\text{otherwise},\end{cases}\\
\operatorname{abs}(A)&=\begin{cases}[0,\max(-a,b)]&a\le0\le b\\\left[\min(|a|,|b|),\max(|a|,|b|)\right]&\text{otherwise}.\end{cases}
\end{aligned}
$$

$$
\{\min_i x_i:x_i\in[\ell_i,u_i]\}\subseteq[\min_i\ell_i,\min_i u_i],\qquad
\{\max_i x_i:x_i\in[\ell_i,u_i]\}\subseteq[\max_i\ell_i,\max_i u_i].
$$

The last two formulas are outer enclosures for a finite nonempty family. Empty families are undefined. Take square roots only over the nonnegative domain, rounding outwards.

**K5: classical probes.** Check satisfiability of $\Gamma$ first; do not score an inconsistent classical context. Only for a consistent context use the following tests; if both sides are SAT the outcome is undetermined. Solver reports, rechecked models, checked proofs, and finite enumeration have different evidentiary status. ASP, temporal, and four-valued probes retain their own semantics.

$$
\Gamma\models\varphi\iff\operatorname{UNSAT}(\Gamma\land\neg\varphi),\qquad
\Gamma\models\neg\varphi\iff\operatorname{UNSAT}(\Gamma\land\varphi).
$$

**K6, K7: description length and precision.** K6 counts the encoded bits of a self-contained typed term, decoder, and dictionary, not Kolmogorov complexity. K7 uses reduced rationals and outward intervals; $1\le p\le256$ with reference precision 64. The integer $q$ below sets the grid lower bound for a square root. Perfect rational squares can be Exact; otherwise retain an enclosure, or ResourceExhausted on budget failure.

$$
x=n/d\ge0,\quad q=\operatorname{isqrt}\!\left(\left\lfloor n2^{2p}/d\right\rfloor\right),\quad
\sqrt{x}\in[q/2^p,(q+1)/2^p].
$$

$$
\begin{aligned}
r&=\min\{r\in\mathbb Z_{\ge0}:x/2^r\le1/2\},\quad y=x/2^r,\quad P=p+r+16,\\
T_m(y)&=\sum_{k=0}^{m}(-y)^k/k!,\quad T_{2j+1}(y)\le e^{-y}\le T_{2j}(y),\\
\operatorname{down}_P(v)&=\lfloor v2^P\rfloor/2^P,\quad \operatorname{up}_P(v)=\lceil v2^P\rceil/2^P.
\end{aligned}
$$

Reduce the Taylor-bound gap to $2^{-P}$ or less and round outwards. Square $r$ times using $[l,u]\mapsto[\operatorname{down}_P(l^2),\operatorname{up}_P(u^2)]$ and check for final width at most $2^{-p}$. If necessary, increase $P$ by 32 and recompute from the rational Taylor bounds up to three times. Reference limits are $r\le64$ and 256 terms. An uncertain small value is never Exact 0.

$$
\begin{aligned}
d_{\min}&=\begin{cases}0&k_L\le K_0\le k_U\\\min(|k_L-K_0|,|k_U-K_0|)&\text{otherwise},\end{cases}\\
d_{\max}&=\max(|k_L-K_0|,|k_U-K_0|),\\
e^{-[x,y]}&\subseteq [\operatorname{lowerExp}(y),\operatorname{upperExp}(x)]\quad (0\le x\le y),\\
Q_{\rm parity}&\in [\operatorname{lowerExp}(d_{\max}/\sigma),\operatorname{upperExp}(d_{\min}/\sigma)],\\
\overline Q&=\min(\operatorname{upperExp}(x),1/(1+x))<1\quad(x=d_{\min}/\sigma>0).
\end{aligned}
$$

Do not automatically clip invalid normalized values. Explicitly registered saturation for a design component is a different rule. An upper bound of one when a $K$ interval includes the baseline is not Exact 1.

<a id="policy-en"></a>

### Selection and search allocation (separate from the scores)

Fix $m<M$ and the direction of each raw RootGoal. Reject an out-of-range exact value instead of clipping; an interval straddling the boundary needs evidence. CR-DESIGN saturation, by contrast, applies only after its AcceptedInputRange check.

$$
g_{\max}(x)=(x-m)/(M-m),\qquad g_{\min}(x)=(M-x)/(M-m).
$$

Check hard conditions first. A maximum-loss gate passes if $u\le\tau$, fails if $l>\tau$; a minimum-gain gate passes if $l\ge\tau$, fails if $u<\tau$. Other cases need evidence. Required preservation uses $\tau=0$. Default JointPareto compares goals, individual losses, primitive gains, and soft costs. It does not add F1/F2 or composite gains on top.

$$
\forall i\in I_{\max}:\ \underline A_i\ge\overline B_i,\qquad
\forall j\in I_{\min}:\ \overline A_j\le\underline B_j.
$$

Conservative dominance requires all these conditions and at least one strict inequality. Overlap does not determine a winner. An original-score winner needs a score lower bound strictly above the upper bound of every other eligible candidate. Missing goals or required representative ordering give NoRecommendation; where comparable, retain one recommendation plus material alternatives.

$$
J=aL_{\rm trans}-bS_j:\qquad
[L_{\rm trans}]J=\begin{cases}a+b\lambda&j=1\\a+bw_6&j=2.\end{cases}
$$

This expansion detects double-counting of the same loss; it is not the default selector.

**Search priority.** $u$ is the maximum active bound width divided by its declared range width (zero contribution for a fixed zero-width range). $B>0$ counts query slots; $\hat c$ is the registered estimated slot cost. Pin the novelty policy first; undefined inputs use canonical round-robin. Reserve $\lceil B/8\rceil$ slots for fair rotation and rank the remainder by descending priority lower bound.

$$
c=\min(1,\hat c/B),\qquad \pi=\frac{(1+u)(1+n)}{1+c},\qquad
n_{\cup}=d_J(F_x,\bigcup_{y\in\mathcal F}F_y),\quad n_{\rm near}=\min_{y\in\mathcal F}d_J(F_x,F_y).
$$

Distance to the union is the reference policy. Nearest-candidate distance is a separate policy that can reverse ordering. K8 below is an alternative exploration proposal only when geometry is registered; it is not physical distance, semantic truth, acceptance or execution authority.

$$
\begin{aligned}
\mathbb B_\kappa&=\{x:\kappa\lVert x\rVert_2^2<1\},\qquad \kappa>0,\quad \kappa\lVert x\rVert_2^2\le1-\mu,\quad 0<\mu<1,\\
t&=\frac{\kappa\lVert x-y\rVert_2^2}{(1-\kappa\lVert x\rVert_2^2)(1-\kappa\lVert y\rVert_2^2)},\qquad q=\sqrt{t/(1+t)},\\
d_\kappa(x,y)&=\frac{\operatorname{acosh}(1+2t)}{\sqrt\kappa}
=\frac{2\operatorname{atanh}(q)}{\sqrt\kappa},\\
T_N(q)&=\sum_{k=0}^{N}\frac{q^{2k+1}}{2k+1},\qquad
\operatorname{atanh}(q)\in\left[T_N(q_L),\,T_N(q_U)+\frac{q_U^{2N+3}}{(2N+3)(1-q_U^2)}\right].
\end{aligned}
$$

Verify the whole coordinate box stays inside the margin; use K4's dedicated square for each term of a squared norm. Require numerical slack $\le2^{-64}$ separately at endpoints $t_L,t_U$. Start internal precision at 80, increase by 32 to at most 256, and use $N=0\ldots256$. If $q_U=1$ or slack fails, refine; at the cap retain ResourceExhausted with a valid bound, otherwise NoScalar. Width due to input uncertainty is not numerical rounding error.

$$
d_{\rm near}=\min_{y\in\mathcal F}d_\kappa(x,y)\in[d_L,d_U],\qquad
n_{\rm geo}\in[1-\operatorname{upperExp}(d_L/s_{\rm geo}),\,1-\operatorname{lowerExp}(d_U/s_{\rm geo})],\quad s_{\rm geo}>0.
$$

Without a registered embedding and a verified frontier, do not manufacture this value. Geometry and distillation proposals cannot change a metric without discrete validation and remeasurement.

<a id="examples-en"></a>

### Numerical examples and verification references

Two probes with weights 1 and 3 and losses 0 and 1 give EpiMapLoss $3/4$. If the second is defined but unobserved, the result is $[0,3/4]$, not a definite 0 or $3/4$. With uniform coefficients the synthetic identity examples give F1 $1/6$ and F2 $5/32$; each differs from itself by zero. These are arithmetic fixtures, not measurements of development effectiveness.

<details>

<summary>All 37 numerical and boundary examples in the fixed contract (G01–G37)</summary>

| ID | Inputs and expected values (fixtures) |
| --- | --- |
| G01 | K1 weights (1,3), losses (0,1) →3/4 |
| G02 | Same weights, losses (0,Unknown[0,1]) →[0,3/4] |
| G03 | K3 {a,b}/{b,c}, weights of 1 →2/3 |
| G04 | Paired baseline (1,0), candidate (1,1) →1/2 |
| G05 | T: Dpar=1/4,Dsem=1,A=1/2 →1/4 |
| G06 | C_match: K0=80,κ=80,K=160 →1/2 |
| G07 | Q: K0=160,σ=80,K=80 and 240 →both exp(-1)≈0.36787944117144232160 |
| G08 | Q: K∈[80,240],K0=160,σ=80 →[exp(-1),1] |
| G09 | Uniform synthetic identity, A=C=1, archive includes exact baseline so N=1, Dctx=Dsem=L=0 →S1=1/6 |
| G10 | Uniform identity, Preserve=1,Q=1,Rebind=Reframe=BreakCost=Redundancy=0,T=G=U=L=Rcounter=Bmem=0 →S2=5/32 |
| G11 | G10 with Redundancy=1/4 →Gstruct=−1/20,S2=3/20 |
| G12 | Raw delta S is S(z) minus measured S(z0); identity delta is 0 even if raw score is not |
| G13 | Loss [0.1,0.2],τ=0.2 passes; [0.2,0.3] needs evidence; [0.21,0.3] fails |
| G14 | c=3,κ_B=1 →Bmem=3/4; proven no path →1+Disconnected |
| G15 | Δcompression K0=80,K=40 with preservation pass →1/2; failed preservation →Ineligible |
| G16 | Brier baseline p=0,y=1, candidate p=1/2 →Δpredict=3/4 |
| G17 | K4 `[-2,3]×[-4,5]` →`[-12,15]`; divisor interval containing 0 →Undefined/DivisionByZero |
| G18 | Error e=5, epsilon=2, r=10 (same unit) →ell=3/10; canonicalized identity 1s→1000ms has error 0 |
| G19 | UNSAT report without checked proof/enumeration →ValidationBasis=SolverReported, not independent validation |
| G20 | Q, K∈[200,240],K0=160,σ=80 →[exp(-1),exp(-1/2)] |
| G21 | Poincare κ=1, x=(0,0), y=(1/2,0) →d=acosh(5/3)=ln3≈1.098612288668109691 |
| G22 | No comparable registered RootGoalProfile →NoRecommendation; retain alternatives |
| G23 | Poincare κ=1 on one diameter: d(0,1/4)=ln(5/3), d(1/4,1/2)=ln(9/5), d(0,1/2)=ln3; the first two sum to the third. Node-ID relabeling leaves distances unchanged |
| G24 | Goal raw range [10,30], v=15 →max score 1/4, min score 3/4; v=31→InvalidInput |
| G25 | Two tied branches A<B by canonical key; relabel IDs leaves dispatch unchanged. Next budget epoch begins after persisted cursor even when frontier revision changed |
| G26 | CR-DESIGN accepted domain [-100,100], normalize [0,10], maximize x=12 →1 with SaturatedHigh; x=101→InvalidInput |
| G27 | B_mem: two paths with cost intervals [1,2]+[1,2] and [3,4] →minimum cost [2,4]; κ_B=2 gives B_mem=[1/2,2/3]. Open graph/no path observation →Unknown, not Disconnected |
| G28 | K4 family min{[0,10],[5,6]} enclosure →[0,6]; max →[5,10]; empty family min/max →Undefined/NoApplicable |
| G29 | Noncrossing Q with dmin/σ=2^-1000: outward exp bound may equal 1, but rational upper `1/(1+2^-1000)<1` retains strict noncrossing |
| G30 | Poincare κ=1, x=(0,0), y=(1/2,1/2), μ=1/4: q=sqrt(1/2); K8 refines internal precision above 64 bits until endpoint numerical slack<=2^-64 or reports ResourceExhausted |
| G31 | `d_near=[0,1]`, `s_geo=1`: `n_geo` encloses `[0,1−e^-1]` with ordered endpoints |
| G32 | Poincare κ=1, x=(1/2,0), y=(0,1/2): exact `t=8/9`, `q²=8/17`; K8 raises precision above 64 bits or reports bounded ResourceExhausted, never false convergence |
| G33 | Poincare κ=1, x∈[0,1/10], y=1/2: the sound enclosure includes `[ln(27/11),ln3]`, width `ln(11/9)`; input uncertainty is not numerical failure |
| G34 | `square([-1/10,1/10])` returns `[0,1/100]` and cannot create a negative norm-square lower bound |
| G35 | Frontier `{{a},{b}}`: proposal `{a}` vs `{a,b}` gives union-distance `1/2,0` and nearest-verified-candidate distance `0,1/2`; bind each to a different SchedulerNoveltyPolicyId |
| G36 | A new feature outside the fixed K3 universe remains InvalidInput/UniverseMismatch until a new ComparisonScope is established |
| G37 | Missing/unverified EmbeddingPolicy remains Unsupported; a search proposal cannot create authority or exact semantic novelty |

</details>

[README](../README.md) · [Fixed measurement contract](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) · [Beta 6.9 snapshot record](epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md)
